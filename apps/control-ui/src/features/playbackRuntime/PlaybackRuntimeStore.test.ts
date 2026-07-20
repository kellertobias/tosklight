import { describe, expect, it } from "vitest";
import type { PlaybackOutcome } from "./contracts";
import { playbackIdentity } from "./contracts";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	DESK_ID,
	deskProjection,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";

function outcome(
	projection = cueProjection(),
	eventSequence: number | null = 12,
	related: PlaybackOutcome["related"] = [],
): PlaybackOutcome {
	const playbackNumber = projection.playback_number ?? 1;
	return {
		request_id: "request-1",
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: { kind: "playback", playback_number: playbackNumber },
		resolved: {
			kind: "playback",
			playback_number: playbackNumber,
			page: 1,
			slot: 1,
		},
		outcome: { status: "applied" },
		durability: "durable",
		projection,
		related,
		desk: null,
		event_sequence: eventSequence,
		desk_event_sequence: null,
		replayed: false,
	};
}

function masterProjection(playbackNumber: number, master: number) {
	const projection = cueProjection(playbackNumber);
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("fixture must contain a running Cuelist");
	return {
		...projection,
		runtime: { ...projection.runtime, master, fader_position: master },
	};
}

function enabledProjection(playbackNumber: number, enabled: boolean) {
	const projection = cueProjection(playbackNumber);
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("fixture must contain a running Cuelist");
	return {
		...projection,
		runtime: { ...projection.runtime, enabled },
	};
}

describe("PlaybackRuntimeStore", () => {
	it("does not let an older repair snapshot overwrite a newer event", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity], 10), [identity]);
		store.applyProjection(cueProjection(1, 2), 12);
		store.installSnapshot(
			playbackSnapshot([identity], 11, [cueProjection(1, 0)]),
			[identity],
		);
		const projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(projection?.target).toBe("cue_list");
		if (projection?.target === "cue_list")
			expect(projection.runtime?.cue_index).toBe(2);
	});

	it("publishes a deterministic master optimistically and rolls it back", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const token = store.beginOptimisticMaster(1, 0.35);
		let projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.master,
		).toBe(0.35);
		store.rollbackProjection(token, new Error("offline"));
		projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.master,
		).toBe(1);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
	});

	it("tracks simultaneous faders independently when they settle out of order", () => {
		const store = new PlaybackRuntimeStore();
		const identities = [playbackIdentity(1), playbackIdentity(2)];
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot(identities), identities);
		const first = store.beginOptimisticMaster(1, 0.25);
		const second = store.beginOptimisticMaster(2, 0.4);

		store.installOutcome(outcome(masterProjection(2, 0.4), 13), second);
		expect(store.getSnapshot().pendingKeys).toEqual(new Set(["playback:1"]));
		store.rollbackProjection(first, new Error("first fader rejected"));

		const firstProjection = store
			.getSnapshot()
			.projections.get("playback:1")?.[0];
		const secondProjection = store
			.getSnapshot()
			.projections.get("playback:2")?.[0];
		expect(
			firstProjection?.target === "cue_list" && firstProjection.runtime?.master,
		).toBe(1);
		expect(
			secondProjection?.target === "cue_list" &&
				secondProjection.runtime?.master,
		).toBe(0.4);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
	});

	it("rolls a newer fader back to an older authoritative outcome", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const first = store.beginOptimisticMaster(1, 0.2);
		const second = store.beginOptimisticMaster(1, 0.6);

		store.installOutcome(outcome(masterProjection(1, 0.2), 13), first);
		let projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.master,
		).toBe(0.6);
		store.rollbackProjection(second, new Error("newer value rejected"));
		projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.master,
		).toBe(0.2);
	});

	it("installs the authoritative action outcome without guessing a cue transition", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		store.installOutcome(outcome(cueProjection(1, 4), 13));
		const projection = store.getSnapshot().projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.cue_index,
		).toBe(4);
		expect(store.getSnapshot().eventSequence).toBe(13);
	});

	it("converges when a related outcome arrives before matching events", () => {
		const store = new PlaybackRuntimeStore();
		const identities = [playbackIdentity(1), playbackIdentity(2)];
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot(identities), identities);
		const token = store.beginOptimisticMaster(2, 0.3);
		let publications = 0;
		store.subscribe(() => publications++);
		const peer = enabledProjection(1, false);
		const primary = masterProjection(2, 0.8);

		store.installOutcome(
			outcome(primary, 12, [{ projection: peer, event_sequence: 11 }]),
			token,
		);
		const afterResponse = store.getSnapshot();

		expect(runtimeFor(store, 1).enabled).toBe(false);
		expect(runtimeFor(store, 2).master).toBe(0.8);
		expect(afterResponse.eventSequence).toBe(12);
		expect(afterResponse.pendingKeys.size).toBe(0);
		expect(publications).toBe(1);
		expect(store.applyProjection(peer, 11)).toBe(false);
		expect(store.applyProjection(primary, 12)).toBe(false);
		expect(store.getSnapshot()).toBe(afterResponse);
	});

	it("settles optimism when related events arrive before the outcome", () => {
		const store = new PlaybackRuntimeStore();
		const identities = [playbackIdentity(1), playbackIdentity(2)];
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot(identities), identities);
		const token = store.beginOptimisticMaster(2, 0.3);
		const peer = enabledProjection(1, false);
		const primary = masterProjection(2, 0.8);

		store.applyProjection(peer, 11);
		store.applyProjection(primary, 12);
		expect(runtimeFor(store, 2).master).toBe(0.3);
		store.installOutcome(
			outcome(primary, 12, [{ projection: peer, event_sequence: 11 }]),
			token,
		);

		expect(runtimeFor(store, 1).enabled).toBe(false);
		expect(runtimeFor(store, 2).master).toBe(0.8);
		expect(store.getSnapshot().eventSequence).toBe(12);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
	});

	it("ignores a late multi-projection outcome after the show scope changes", () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([playbackIdentity(1)]), [
			playbackIdentity(1),
		]);
		store.beginOptimisticMaster(1, 0.3);
		store.reset("00000000-0000-0000-0000-000000000099", DESK_ID);
		const current = store.getSnapshot();
		let emissions = 0;
		store.subscribe(() => emissions++);

		store.installOutcome(
			outcome(masterProjection(1, 0.8), 12, [
				{ projection: enabledProjection(2, false), event_sequence: 11 },
			]),
		);

		expect(store.getSnapshot()).toBe(current);
		expect(emissions).toBe(0);
	});

	it("invalidates pending requests when the authority changes in the same show and desk", () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([playbackIdentity(1)]), [
			playbackIdentity(1),
		]);
		const token = store.beginOptimisticMaster(1, 0.3);

		store.reset(SHOW_ID, DESK_ID, "authority-b");
		const current = store.getSnapshot();
		let emissions = 0;
		store.subscribe(() => emissions++);
		store.installOutcome(outcome(masterProjection(1, 0.8), 12), token);

		expect(store.getSnapshot()).toBe(current);
		expect(store.getSnapshot().projections.size).toBe(0);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(emissions).toBe(0);
	});

	it("keeps a newer event over a delayed no-change response", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const token = store.beginRequest(identity);

		store.applyProjection(cueProjection(1, 4), 20);
		store.installOutcome(
			{
				...outcome(cueProjection(1, 0), null),
				outcome: { status: "no_change" },
			},
			token,
		);

		expect(runtimeFor(store, 1).cue_index).toBe(4);
		expect(store.getSnapshot().eventSequence).toBe(20);
	});

	it.each([
		["no-change", { status: "no_change" } as const],
		[
			"captured",
			{ status: "captured", pending: "go" } as const,
		],
	])("keeps a newer desk event over a delayed %s response", (_label, status) => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const token = store.beginRequest(identity);

		store.applyDesk(deskProjection(3), 11);
		store.installOutcome(
			{
				...outcome(cueProjection(1, 0), null),
				outcome: status,
				desk: deskProjection(1),
			},
			token,
		);

		expect(store.getSnapshot().desk?.active_page).toBe(3);
		expect(store.getSnapshot().eventSequence).toBe(11);
	});

	it("rolls back a rejected page and retains an accepted page sequence", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const rejected = store.beginOptimisticPage(2);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
		store.rollbackPage(rejected, new Error("conflict"));
		expect(store.getSnapshot().desk?.active_page).toBe(1);
		const accepted = store.beginOptimisticPage(3);
		store.commitPage(accepted, 3, 15);
		expect(store.getSnapshot().desk?.active_page).toBe(3);
		expect(store.getSnapshot().error).toBeNull();
	});

	it("keeps an optimistic page across an unrelated identity snapshot", () => {
		const store = new PlaybackRuntimeStore();
		const firstIdentity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([firstIdentity]), [firstIdentity]);
		const pending = store.beginOptimisticPage(2);

		const secondIdentity = playbackIdentity(2);
		store.installSnapshot(playbackSnapshot([secondIdentity], 11), [
			secondIdentity,
		]);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
		store.commitPage(pending, 2, 12);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
	});

	it("merges selection events under a pending page and rolls back to authority", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const pending = store.beginOptimisticPage(3);

		store.applyDesk({ ...deskProjection(1), selected_playback: 7 }, 11);
		expect(store.getSnapshot().desk).toMatchObject({
			active_page: 3,
			selected_playback: 7,
		});
		store.rollbackPage(pending, new Error("page rejected"));
		expect(store.getSnapshot().desk).toMatchObject({
			active_page: 1,
			selected_playback: 7,
		});
	});

	it("settles page outcomes and failures in request order", () => {
		const store = new PlaybackRuntimeStore();
		const identity = playbackIdentity(1);
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(playbackSnapshot([identity]), [identity]);
		const first = store.beginOptimisticPage(2);
		const second = store.beginOptimisticPage(3);

		store.commitPage(first, 2, 11);
		expect(store.getSnapshot().desk?.active_page).toBe(3);
		store.rollbackPage(second, new Error("newer page rejected"));
		expect(store.getSnapshot().desk?.active_page).toBe(2);

		const older = store.beginOptimisticPage(4);
		const newer = store.beginOptimisticPage(5);
		store.commitPage(newer, 5, 12);
		store.rollbackPage(older, new Error("stale failure"));
		expect(store.getSnapshot().desk?.active_page).toBe(5);
	});
});

function runtimeFor(store: PlaybackRuntimeStore, playbackNumber: number) {
	const projection = store
		.getSnapshot()
		.projections.get(`playback:${playbackNumber}`)?.[0];
	if (projection?.target !== "cue_list" || !projection.runtime)
		throw new Error("expected a running Cuelist projection");
	return projection.runtime;
}
