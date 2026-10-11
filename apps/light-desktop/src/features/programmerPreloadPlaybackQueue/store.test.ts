import { describe, expect, it } from "vitest";
import { ProgrammerPreloadPlaybackQueueStore } from "./store";
import {
	AUTHORITY_A,
	queuedPlayback,
	queueProjection,
	queueSnapshot,
	SHOW_ID,
	SESSION_ID,
} from "./testFixtures";

function hydratedStore() {
	const store = new ProgrammerPreloadPlaybackQueueStore();
	store.reset(SHOW_ID, SESSION_ID, AUTHORITY_A);
	store.installSnapshot(queueSnapshot());
	return store;
}

describe("ProgrammerPreloadPlaybackQueueStore", () => {
	it("preserves ordered duplicate actions exactly", () => {
		const actions = [
			queuedPlayback({
				address: { page: 1, position: 9 },
				action: "back",
				controlSource: { surface_id: "osc:test", control_id: 7 },
			}),
			queuedPlayback({
				address: { page: 1, position: 7 },
				action: "go",
				controlSource: { surface_id: "matter:test", control_id: 7 },
			}),
			queuedPlayback({
				address: { page: 1, position: 9 },
				action: "back",
				controlSource: { surface_id: "osc:test", control_id: 7 },
			}),
		];
		const store = new ProgrammerPreloadPlaybackQueueStore();
		store.reset(SHOW_ID, SESSION_ID, AUTHORITY_A);

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

	it("rejects an authority with a malformed revision", () => {
		const store = hydratedStore();
		expect(() =>
			store.applyProjection(queueProjection({ revision: -1 }), 11),
		).toThrow(/revision/);
		expect(store.getSnapshot()).toMatchObject({
			projection: { revision: 2 },
			repairRequired: true,
		});
	});
});
