import { describe, expect, it } from "vitest";
import {
	cueProjection,
	SHOW_ID,
	CUE_LIST_ID,
} from "../features/playbackRuntime/testFixtures";
import type {
	PlaybackRuntimeIdentity,
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
