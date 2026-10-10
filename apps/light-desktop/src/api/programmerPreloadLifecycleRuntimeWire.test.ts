import { describe, expect, it } from "vitest";
import {
	cueProjection,
	SHOW_ID,
	CUE_LIST_ID,
} from "../features/playbackRuntime/testFixtures";
import type {
	PlaybackRuntimeIdentity,
	DynamicPlaybackRuntimeProjection,
	ProgrammingPreloadLifecycleOutcome as WireOutcome,
} from "./generated/light-wire";
import type { ProgrammerPreloadLifecycleRequest } from "../features/programmerPreloadLifecycle/contracts";
import { decodeProgrammerPreloadLifecycleOutcome } from "./programmerPreloadLifecycleWire";

const request: ProgrammerPreloadLifecycleRequest = {
	requestId: "preload-go-owners",
	expectedCaptureModeRevision: 3,
	expectedValuesRevision: 5,
	expectedQueueRevision: 6,
	expectedSelectionRevision: 7,
	action: {
		type: "go",
		showId: SHOW_ID,
		expectedShowRevision: 4,
		expectedPlaybackEventSequence: 10,
	},
};
function response(): WireOutcome {
	const physical = cueProjection(11);
	const virtual = cueProjection(1301);
	if (
		physical.target !== "cue_list" ||
		!physical.runtime ||
		virtual.target !== "cue_list" ||
		!virtual.runtime
	)
		throw new Error("running fixture required");
	physical.runtime.owner = { kind: "playback", playback_number: 11 };
	virtual.requested = { kind: "virtual", page: 2, playback_number: 1301 };
	virtual.runtime.owner = { kind: "virtual", page: 2, playback_number: 1301 };
	return {
		request_id: request.requestId,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		replayed: false,
		status: "changed",
		active: false,
		capture_mode: {
			revision: 4,
			blind: false,
			preview: false,
			preload_capture_programmer: true,
		},
		capture_mode_event_sequence: 9,
		values_revision: 5,
		queue_revision: 7,
		queue_projection: { revision: 7, actions: [] },
		queue_event_sequence: 13,
		selection_revision: 7,
		commit: {
			show_id: SHOW_ID,
			show_revision: 4,
			playback_event_sequence_before: 10,
			playback_event_sequence_after: 12,
			committed_at: "2026-10-10T01:28:00Z",
			programmer_fade_millis: 0,
			executed_playback_actions: 2,
			executed: [
				{ playback_number: 11, page: 2, action: "on", surface: "physical" },
				{
					playback_number: 1301,
					page: 2,
					action: "toggle",
					surface: "virtual",
				},
			],
			runtime_changes: [
				{ projection: physical, event_sequence: 11 },
				{ projection: virtual, event_sequence: 12 },
			],
		},
	};
}
function firstRuntime(body: WireOutcome) {
	const projection = body.commit!.runtime_changes[0].projection;
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("running fixture required");
	return projection;
}

describe("Preload commit exact runtime owners", () => {
	it("accepts physical and virtual owners in one realistic committed GO response", () => {
		const body = response();
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
		expect(decoded.commit?.executedPlaybackActions).toBe(2);
		expect(
			decoded.commit?.runtimeChanges.map((item) => item.eventSequence),
		).toEqual([11, 12]);
		const changes = decoded.commit!.runtimeChanges;
		expect(changes[0].projection).toMatchObject({
			requested: { kind: "playback", playback_number: 11 },
			runtime: { owner: { kind: "playback", playback_number: 11 } },
		});
		expect(changes[1].projection).toMatchObject({
			requested: { kind: "virtual", page: 2, playback_number: 1301 },
			runtime: { owner: { kind: "virtual", page: 2, playback_number: 1301 } },
		});
		expect(body.commit?.runtime_changes[0].projection).toEqual(
			changes[0].projection,
		);
	});
	it.each([
		undefined,
		null,
	])("accepts legacy missing/null owner %s", (owner) => {
		const body = response();
		for (const change of body.commit!.runtime_changes) {
			if (change.projection.target !== "cue_list" || !change.projection.runtime)
				throw new Error("cue");
			if (owner === undefined) delete change.projection.runtime.owner;
			else change.projection.runtime.owner = null;
		}
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).not.toThrow();
	});
	it("keeps an aggregate requested identity distinct from exact Direct activation owner", () => {
		const body = response();
		const projection = firstRuntime(body);
		projection.requested = { kind: "cue_list", cue_list_id: CUE_LIST_ID };
		projection.runtime!.owner = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request)
			.commit!.runtimeChanges[0].projection;
		expect(decoded).toMatchObject({
			requested: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			runtime: { owner: { kind: "direct_cue_list", cue_list_id: CUE_LIST_ID } },
		});
	});
	it("accepts a declared Direct request without changing its exact owner", () => {
		const body = response();
		const projection = firstRuntime(body);
		projection.requested = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		projection.runtime!.owner = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		expect(
			decodeProgrammerPreloadLifecycleOutcome(body, request).commit!
				.runtimeChanges[0].projection,
		).toMatchObject({
			requested: projection.requested,
			runtime: { owner: projection.runtime!.owner },
		});
	});
	it("does not misclassify a GO commit as a successful pending Clear", () => {
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(response(), {
				...request,
				action: { type: "clear_pending" },
			}),
		).toThrow(/\$\.commit: expected absent/);
	});

	it.each([
		{ kind: "playback", playback_number: 0 },
		{ kind: "virtual", page: 0, playback_number: 1301 },
		{ kind: "virtual", page: 2, playback_number: 0 },
		{ kind: "undeclared" },
		{ kind: "playback", playback_number: 11, foreign: true },
		{ kind: "direct_cue_list", cue_list_id: CUE_LIST_ID, playback_number: 11 },
		{ kind: "virtual", page: 2 },
	])("rejects malformed/foreign owner %#", (owner) => {
		const body = response();
		firstRuntime(body).runtime!.owner =
			owner as unknown as PlaybackRuntimeIdentity;
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/\$\.commit\.runtime_changes\[0\]\.projection\.runtime\.owner/);
	});
	it("continues rejecting undeclared runtime fields", () => {
		const body = response();
		Object.assign(firstRuntime(body).runtime!, { foreign: true });
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/runtime\.foreign/);
	});
});

function dynamicResponse(active: boolean, embedded: boolean): WireOutcome {
	const body = response();
	for (const change of body.commit!.runtime_changes) {
		const original = change.projection;
		const runtime: DynamicPlaybackRuntimeProjection = {
			playback_number: original.playback_number!,
			enabled: true,
			paused: false,
			flash: false,
			activated_at: "2026-10-10T01:28:00Z",
			fader_value: 1,
			fader_pickup_required: true,
			fader_pickup_target: 0.5,
			size: 0.75,
			master: 1,
			local_speed_numerator: 1,
			local_speed_denominator: 2,
			learned_duration_millis: null,
			state: "active",
			instance_id: "66666666-6666-4666-8666-666666666666",
			controller_id: "77777777-7777-4777-8777-777777777777",
			winning_controller_id: "77777777-7777-4777-8777-777777777777",
			controller_status: "winning",
			target_count: 4,
			compatible_target_count: 3,
			missing_target_count: 0,
			unpatched_target_count: 1,
			lane_count: 2,
			supported_address_count: 6,
			skipped_address_count: 2,
			speed_source: "speed_group",
			effective_speed_multiplier: 0.5,
			effective_duration_millis: 4000,
			warning: "One target is unpatched",
		};
		change.projection = {
			scope: original.scope,
			requested: original.requested,
			playback_number: original.playback_number,
			target: "dynamic",
			dynamic_id: embedded ? null : "88888888-8888-4888-8888-888888888888",
			last_known_pool_number: 100,
			embedded,
			runtime: active ? runtime : null,
		};
	}
	return body;
}

describe("Preload committed standalone Dynamic projections", () => {
	it.each([
		[true, false],
		[true, true],
		[false, false],
		[false, true],
	])("preserves physical/virtual identity and exact Dynamic runtime active=%s embedded=%s", (active, embedded) => {
		const body = dynamicResponse(active, embedded);
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
		expect(decoded.commit!.executedPlaybackActions).toBe(2);
		expect(
			decoded.commit!.runtimeChanges.map((change) => change.eventSequence),
		).toEqual([11, 12]);
		expect(
			decoded.commit!.runtimeChanges.map((change) => change.projection),
		).toEqual(body.commit!.runtime_changes.map((change) => change.projection));
		expect(decoded.commit!.runtimeChanges[0].projection.requested).toEqual({
			kind: "playback",
			playback_number: 11,
		});
		expect(decoded.commit!.runtimeChanges[1].projection.requested).toEqual({
			kind: "virtual",
			page: 2,
			playback_number: 1301,
		});
	});
	it.each([
		["projection", { foreign: true }, "foreign"],
		["projection", { embedded: "yes" }, "embedded"],
		["projection", { last_known_pool_number: 0 }, "last_known_pool_number"],
		["runtime", { foreign: true }, "foreign"],
		["runtime", { owner: { kind: "playback", playback_number: 11 } }, "owner"],
		["runtime", { state: "undeclared" }, "state"],
		["runtime", { playback_number: 0 }, "playback_number"],
		["runtime", { target_count: -1 }, "target_count"],
		["runtime", { controller_id: null }, "controller_id"],
		["runtime", { fader_pickup_required: "yes" }, "fader_pickup_required"],
	])("rejects malformed/foreign Dynamic %s payload %#", (part, patch, field) => {
		const body = dynamicResponse(true, false);
		const projection = body.commit!.runtime_changes[0].projection;
		if (projection.target !== "dynamic" || !projection.runtime)
			throw new Error("Dynamic fixture required");
		Object.assign(part === "runtime" ? projection.runtime : projection, patch);
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(
			new RegExp(
				`\\$\\.commit\\.runtime_changes\\[0\\]\\.projection.*${field}`,
			),
		);
	});
});
