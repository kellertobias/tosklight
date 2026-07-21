import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackOutcome,
	PlaybackProjection,
} from "../playbackRuntime/contracts";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import {
	CUE_LIST_ID,
	cueProjection,
	DESK_ID,
	playbackSnapshot,
	SHOW_ID,
} from "../playbackRuntime/testFixtures";
import type { ServerController } from "./model";
import { createPlaybackRuntimeActions } from "./playbackRuntime";

function outcome(projection: PlaybackProjection): PlaybackOutcome {
	return {
		request_id: "request-1",
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: { kind: "playback", playback_number: 1 },
		resolved: {
			kind: "playback",
			playback_number: 1,
			page: 1,
			slot: 1,
		},
		outcome: { status: "applied" },
		durability: "durable",
		projection,
		related: [],
		desk: null,
		event_sequence: 12,
		desk_event_sequence: null,
		replayed: false,
	};
}

describe("Playback runtime actions", () => {
	it("binds cue-list requests to the active authority", async () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([]), []);
		const resolvers: Array<(value: PlaybackOutcome) => void> = [];
		const playbackRuntimeAction = vi.fn(
			() =>
				new Promise<PlaybackOutcome>((resolve) => {
					resolvers.push(resolve);
				}),
		);
		const setError = vi.fn();
		const model = {
			bootstrap: { active_show: { id: SHOW_ID } },
			client: { playbackRuntimeAction },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError,
		} as unknown as ServerController;
		const beginRequest = vi.spyOn(store, "beginRequest");
		const actions = createPlaybackRuntimeActions(model);

		const cueListPending = actions.playbackAction(CUE_LIST_ID, "go");
		expect(beginRequest).toHaveBeenNthCalledWith(1, {
			kind: "cue_list",
			cue_list_id: CUE_LIST_ID,
		});
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		for (const resolve of resolvers) resolve(outcome(cueProjection()));

		await cueListPending;
		expect(store.getSnapshot().projections.size).toBe(0);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(setError).not.toHaveBeenCalled();
	});

	it("ignores a late runtime failure after same-scope authority replacement", async () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([]), []);
		let rejectOutcome: (reason: Error) => void = () => undefined;
		const playbackRuntimeAction = vi.fn(
			() =>
				new Promise<PlaybackOutcome>((_resolve, reject) => {
					rejectOutcome = reject;
				}),
		);
		const setError = vi.fn();
		const model = {
			bootstrap: { active_show: { id: SHOW_ID } },
			client: { playbackRuntimeAction },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError,
		} as unknown as ServerController;
		const actions = createPlaybackRuntimeActions(model);

		const request = actions.playbackAction(CUE_LIST_ID, "go");
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		rejectOutcome(new Error("old authority failed"));
		await request;

		expect(store.getSnapshot().error).toBeNull();
		expect(setError).not.toHaveBeenCalled();
	});
});
