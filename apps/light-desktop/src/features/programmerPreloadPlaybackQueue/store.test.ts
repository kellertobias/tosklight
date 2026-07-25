import { describe, expect, it } from "vitest";
import { ProgrammerPreloadPlaybackQueueStore } from "./store";
import {
	AUTHORITY_A,
	queuedPlayback,
	queueProjection,
	queueSnapshot,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";

function hydratedStore() {
	const store = new ProgrammerPreloadPlaybackQueueStore();
	store.reset(SHOW_ID, USER_ID, AUTHORITY_A);
	store.installSnapshot(queueSnapshot());
	return store;
}

describe("ProgrammerPreloadPlaybackQueueStore", () => {
	it("preserves ordered duplicate actions exactly", () => {
		const actions = [
			queuedPlayback({ playbackNumber: 9, action: "back", surface: "osc" }),
			queuedPlayback({ playbackNumber: 7, action: "go", surface: "matter" }),
			queuedPlayback({ playbackNumber: 9, action: "back", surface: "osc" }),
		];
		const store = new ProgrammerPreloadPlaybackQueueStore();
		store.reset(SHOW_ID, USER_ID, AUTHORITY_A);

		store.installSnapshot(queueSnapshot({ actions }));

		expect(store.getSnapshot().projection?.actions).toEqual(actions);
		expect(store.getSnapshot().projection?.actions).not.toBe(actions);
	});

	it("accepts filtered cursor jumps but requires contiguous revisions", () => {
		const store = hydratedStore();
		store.applyProjection(
			queueProjection({
				revision: 3,
				actions: [queuedPlayback(), queuedPlayback()],
			}),
			42,
		);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 42,
			projection: { revision: 3 },
		});

		expect(() =>
			store.applyProjection(queueProjection({ revision: 5 }), 43),
		).toThrow(/revision is not contiguous/);
		expect(store.getSnapshot().repairRequired).toBe(true);
	});

	it("rejects foreign-user authority", () => {
		const store = hydratedStore();
		expect(() =>
			store.applyProjection(
				queueProjection({ userId: "operator-b", revision: 3 }),
				11,
			),
		).toThrow(/active user/);
		expect(store.getSnapshot()).toMatchObject({
			projection: { userId: USER_ID, revision: 2 },
			repairRequired: true,
		});
	});
});
