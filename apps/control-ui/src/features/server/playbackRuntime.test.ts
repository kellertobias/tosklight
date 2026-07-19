import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackOutcome,
	PlaybackProjection,
} from "../playbackRuntime/contracts";
import { playbackIdentity } from "../playbackRuntime/contracts";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import {
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
});
