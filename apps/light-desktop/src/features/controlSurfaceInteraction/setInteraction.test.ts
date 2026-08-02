import { describe, expect, it } from "vitest";
import type {
	ControlSurfaceInteractionScope,
	GroupInteractionIdentity,
	PlaybackInteractionIdentity,
} from "./contracts";
import {
	initialSetInteractionState,
	transitionSetInteraction,
} from "./setInteraction";

const scope: ControlSurfaceInteractionScope = {
	deskId: "desk-main",
	showId: "show-a",
	surfaceId: "group-pool-main",
};
const group: GroupInteractionIdentity = {
	objectId: "4",
	objectRevision: 12,
};
const currentPlayback: PlaybackInteractionIdentity = {
	addressing: "current_page",
	pageNumber: 3,
	slot: 2,
	pageObjectId: "page-3",
	pageObjectRevision: 8,
	playbackObjectId: "playback-17",
	playbackObjectRevision: 5,
};
const explicitPlayback: PlaybackInteractionIdentity = {
	...currentPlayback,
	addressing: "explicit_page",
	pageNumber: 7,
	pageObjectId: "page-7",
};

function arm() {
	return transitionSetInteraction(initialSetInteractionState(scope), {
		type: "arm_set",
		source: "touch",
		scope,
	}).state;
}

function chooseGroup() {
	return transitionSetInteraction(arm(), {
		type: "choose_group",
		source: "touch",
		scope,
		group,
	});
}

describe("scoped SET interaction", () => {
	it("keeps a chosen Group pending until Enter opens its settings", () => {
		const chosen = chooseGroup();

		expect(chosen).toEqual({
			status: "transitioned",
			state: { phase: "group_source_pending", scope, group },
			intent: {
				type: "choose_group_master_source",
				source: "touch",
				scope,
				group,
			},
		});

		const entered = transitionSetInteraction(chosen.state, {
			type: "enter",
			source: "hardware",
			scope,
		});
		expect(entered).toEqual({
			status: "transitioned",
			state: { phase: "idle", scope },
			intent: {
				type: "open_group_settings",
				source: "hardware",
				scope,
				group,
			},
		});
	});

	it.each([
		currentPlayback,
		explicitPlayback,
	])("assigns the pending explicit Group to a $addressing Playback identity", (playback) => {
		const pending = chooseGroup().state;
		const assigned = transitionSetInteraction(pending, {
			type: "choose_playback",
			source: "mouse",
			scope,
			playback,
		});

		expect(assigned).toEqual({
			status: "transitioned",
			state: { phase: "idle", scope },
			intent: {
				type: "assign_group_master",
				source: "mouse",
				scope,
				group,
				playback,
			},
		});
	});

	it("opens Playback settings from SET without reading an incidental Group", () => {
		const opened = transitionSetInteraction(arm(), {
			type: "choose_playback",
			source: "touch",
			scope,
			playback: currentPlayback,
		});

		expect(opened.intent).toEqual({
			type: "open_playback_settings",
			source: "touch",
			scope,
			playback: currentPlayback,
		});
		expect(opened.state.phase).toBe("idle");
	});

	it.each([
		"cancel",
		"clear",
	] as const)("discards a pending Group source on %s", (type) => {
		const cleared = transitionSetInteraction(chooseGroup().state, {
			type,
			scope,
		});
		expect(cleared).toEqual({
			status: "transitioned",
			state: { phase: "idle", scope },
			intent: null,
		});
	});

	it("discards pending authority when the desk, show, or surface scope is replaced", () => {
		const replacement = {
			deskId: "desk-wing",
			showId: "show-b",
			surfaceId: "hardware-controls",
		};
		const replaced = transitionSetInteraction(chooseGroup().state, {
			type: "replace_scope",
			scope: replacement,
		});

		expect(replaced).toEqual({
			status: "transitioned",
			state: { phase: "idle", scope: replacement },
			intent: null,
		});
	});

	it("rejects an event from another scope without consuming the pending source", () => {
		const pending = chooseGroup().state;
		const stale = transitionSetInteraction(pending, {
			type: "choose_playback",
			source: "touch",
			scope: { ...scope, showId: "replacement-show" },
			playback: currentPlayback,
		});

		expect(stale).toEqual({
			status: "scope_mismatch",
			state: pending,
			intent: null,
		});
	});

	it("treats bare SET Enter as a mutation-free cancel", () => {
		const entered = transitionSetInteraction(arm(), {
			type: "enter",
			source: "keyboard",
			scope,
		});
		expect(entered).toEqual({
			status: "transitioned",
			state: { phase: "idle", scope },
			intent: null,
		});
	});

	it.each([
		["select_group_live", "touch"],
		["select_group_frozen", "hardware"],
		["open_group_settings", "context_menu"],
	] as const)("routes direct %s with its exact Group identity", (type, source) => {
		const direct = transitionSetInteraction(chooseGroup().state, {
			type,
			source,
			scope,
			group,
		});

		expect(direct.intent).toEqual({ type, source, scope, group });
		expect(direct.state.phase).toBe("idle");
	});

	it("rejects Group or Playback choices that were not preceded by SET", () => {
		const idle = initialSetInteractionState(scope);
		for (const event of [
			{ type: "choose_group" as const, source: "touch" as const, scope, group },
			{
				type: "choose_playback" as const,
				source: "touch" as const,
				scope,
				playback: currentPlayback,
			},
		])
			expect(transitionSetInteraction(idle, event)).toEqual({
				status: "invalid_transition",
				state: idle,
				intent: null,
			});
	});
});
