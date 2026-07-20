import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackOutcome,
	PlaybackProjection,
} from "../playbackRuntime/contracts";
import { playbackIdentity } from "../playbackRuntime/contracts";
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

function masterProjection(master: number): PlaybackProjection {
	const projection = cueProjection();
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("fixture must contain a running Cuelist");
	return {
		...projection,
		runtime: { ...projection.runtime, master, fader_position: master },
	};
}

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

function master(store: PlaybackRuntimeStore) {
	const projection = store.getSnapshot().projections.get("playback:1")?.[0];
	return projection?.target === "cue_list" ? projection.runtime?.master : null;
}

describe("Playback runtime actions", () => {
	it("updates a master immediately and never reloads the broad v1 snapshot", async () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		let resolveOutcome: (value: PlaybackOutcome) => void = () => undefined;
		const playbackRuntimeAction = vi.fn(
			() =>
				new Promise<PlaybackOutcome>((resolve) => {
					resolveOutcome = resolve;
				}),
		);
		const legacyPlaybacks = vi.fn();
		const model = {
			client: { playbackRuntimeAction, playbacks: legacyPlaybacks },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError: vi.fn(),
		} as unknown as ServerController;
		const actions = createPlaybackRuntimeActions(model);

		const pending = actions.poolPlaybackAction(1, "master", { value: 0.35 });
		expect(master(store)).toBe(0.35);
		expect(playbackRuntimeAction).toHaveBeenCalledOnce();
		expect(legacyPlaybacks).not.toHaveBeenCalled();

		resolveOutcome(outcome(masterProjection(0.35)));
		await pending;
		expect(master(store)).toBe(0.35);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(legacyPlaybacks).not.toHaveBeenCalled();
	});

	it("binds non-optimistic cue-list and pool requests to the active authority", async () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const resolvers: Array<(value: PlaybackOutcome) => void> = [];
		const playbackRuntimeAction = vi.fn(
			() =>
				new Promise<PlaybackOutcome>((resolve) => {
					resolvers.push(resolve);
				}),
		);
		const setError = vi.fn();
		const model = {
			client: { playbackRuntimeAction },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError,
		} as unknown as ServerController;
		const beginRequest = vi.spyOn(store, "beginRequest");
		const actions = createPlaybackRuntimeActions(model);

		const cueListPending = actions.playbackAction(CUE_LIST_ID, "go");
		const poolPending = actions.poolPlaybackAction(1, "go");
		expect(beginRequest).toHaveBeenNthCalledWith(1, {
			kind: "cue_list",
			cue_list_id: CUE_LIST_ID,
		});
		expect(beginRequest).toHaveBeenNthCalledWith(2, identity);
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		for (const resolve of resolvers) resolve(outcome(cueProjection()));

		await Promise.all([cueListPending, poolPending]);
		expect(store.getSnapshot().projections.size).toBe(0);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(setError).not.toHaveBeenCalled();
	});

	it("ignores late page success and failure after same-scope authority replacement", async () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const pending: Array<{
			resolve(value: { event_sequence: number | null }): void;
			reject(reason: Error): void;
		}> = [];
		const setPlaybackPage = vi.fn(
			() =>
				new Promise<{ event_sequence: number | null }>((resolve, reject) => {
					pending.push({ resolve, reject });
				}),
		);
		const setError = vi.fn();
		const model = {
			client: { setPlaybackPage },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError,
		} as unknown as ServerController;
		const actions = createPlaybackRuntimeActions(model);

		const successful = actions.setPlaybackPage(2);
		const failed = actions.setPlaybackPage(3);
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		pending[0].resolve({ event_sequence: 11 });
		pending[1].reject(new Error("old authority failed"));
		await Promise.all([successful, failed]);

		expect(store.getSnapshot().desk).toBeNull();
		expect(store.getSnapshot().error).toBeNull();
		expect(setError).not.toHaveBeenCalled();
	});

	it("ignores a late runtime failure after same-scope authority replacement", async () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		let rejectOutcome: (reason: Error) => void = () => undefined;
		const playbackRuntimeAction = vi.fn(
			() =>
				new Promise<PlaybackOutcome>((_resolve, reject) => {
					rejectOutcome = reject;
				}),
		);
		const setError = vi.fn();
		const model = {
			client: { playbackRuntimeAction },
			session: { desk: { id: DESK_ID } },
			playbacks: null,
			playbackRuntimeStore: store,
			setError,
		} as unknown as ServerController;
		const actions = createPlaybackRuntimeActions(model);

		const request = actions.poolPlaybackAction(1, "go");
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		rejectOutcome(new Error("old authority failed"));
		await request;

		expect(store.getSnapshot().error).toBeNull();
		expect(setError).not.toHaveBeenCalled();
	});
});
