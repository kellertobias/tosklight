import { describe, expect, it, vi } from "vitest";
import { OutputRuntimeStore } from "./store";
import {
	DESK_ID,
	OTHER_DESK_ID,
	OTHER_SHOW_ID,
	outputProjection,
	outputSnapshot,
	SHOW_ID,
} from "./testFixtures";

function readyStore() {
	const store = new OutputRuntimeStore();
	store.reset(SHOW_ID, DESK_ID, "session-a");
	store.installSnapshot(outputSnapshot());
	return store;
}

describe("OutputRuntimeStore", () => {
	it("installs immutable exact-Show authority and rejects stale scopes", () => {
		const store = new OutputRuntimeStore();
		store.reset(SHOW_ID, DESK_ID, "session-a");
		const scope = store.captureScope();
		const projection = outputProjection();

		expect(store.installSnapshot({ cursor: 10, projection }, scope)).toBe(true);
		expect(store.getSnapshot().projection).not.toBe(projection);
		expect(Object.isFrozen(store.getSnapshot().projection)).toBe(true);
		expect(
			store.installSnapshot(outputSnapshot({ showId: OTHER_SHOW_ID }), scope),
		).toBe(false);
		store.reset(SHOW_ID, OTHER_DESK_ID, "session-b");
		expect(store.installSnapshot(outputSnapshot(), scope)).toBe(false);
		expect(store.getSnapshot().projection).toBeNull();
	});

	it("reconciles response-before-event without republishing a duplicate", () => {
		const store = readyStore();
		const listener = vi.fn();
		store.subscribe(listener);
		const changed = outputProjection({
			revision: 2,
			grandMaster: 0.42,
			blackout: true,
		});
		expect(
			store.beginOptimistic({
				requestId: "response-first",
				grandMaster: 0.42,
				blackout: true,
			}),
		).toBe(true);

		expect(store.settleChanged("response-first", changed, 11)).toBe("settled");
		const settled = store.getSnapshot();
		const calls = listener.mock.calls.length;
		expect(store.applyChange({ projection: changed }, 11)).toBe(true);

		expect(listener).toHaveBeenCalledTimes(calls);
		expect(store.getSnapshot()).toBe(settled);
		expect(settled).toMatchObject({
			eventSequence: 11,
			authorityRevision: 2,
			pendingRequestIds: [],
			projection: { grandMaster: 0.42, blackout: true },
		});
	});

	it("keeps combined optimism until an event-before-response is acknowledged", () => {
		const store = readyStore();
		const changed = outputProjection({
			revision: 2,
			grandMaster: 0.3,
			blackout: true,
		});
		store.beginOptimistic({
			requestId: "event-first",
			grandMaster: 0.3,
			blackout: true,
		});
		store.applyChange({ projection: changed }, 11);

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			pendingRequestIds: ["event-first"],
			projection: { grandMaster: 0.3, blackout: true },
		});
		expect(store.settleChanged("event-first", changed, 11)).toBe("settled");
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("settles a replayed no-op without cloning the projection", () => {
		const store = readyStore();
		const authority = store.getSnapshot().projection;
		store.beginOptimistic({ requestId: "same", grandMaster: 1 });

		expect(store.getSnapshot().projection).toBe(authority);
		expect(store.settleNoChange("same", outputProjection())).toBe("settled");
		expect(store.getSnapshot().projection).toBe(authority);
	});

	it("rolls back one action while preserving later optimistic intent", () => {
		const store = readyStore();
		store.beginOptimistic({ requestId: "first", grandMaster: 0.5 });
		store.beginOptimistic({ requestId: "second", blackout: true });

		expect(store.rollback("first", new Error("rejected"))).toBe(true);
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: ["second"],
			projection: { grandMaster: 1, blackout: true },
			error: new Error("rejected"),
		});
	});

	it("requires narrow repair for divergent duplicates and revision jumps", () => {
		const store = readyStore();
		expect(() =>
			store.applyChange(
				{ projection: outputProjection({ grandMaster: 0.2 }) },
				10,
			),
		).toThrow(/sequence conflicts/);
		expect(store.getSnapshot().repairRequired).toBe(true);

		store.installRepairSnapshot(outputSnapshot({ cursor: 12, revision: 2 }));
		expect(() =>
			store.applyChange({ projection: outputProjection({ revision: 4 }) }, 13),
		).toThrow(/revision is not contiguous/);
	});
});
