import { describe, expect, it, vi } from "vitest";
import { ProgrammerPriorityStore } from "./store";
import {
	OTHER_USER_ID,
	priorityProjection,
	prioritySnapshot,
	USER_ID,
} from "./testFixtures";

function readyStore() {
	const store = new ProgrammerPriorityStore();
	store.reset(USER_ID, "session-a");
	store.installSnapshot(prioritySnapshot());
	return store;
}

describe("ProgrammerPriorityStore", () => {
	it("installs immutable exact-user authority and rejects stale scopes", () => {
		const store = new ProgrammerPriorityStore();
		store.reset(USER_ID, "session-a");
		const scope = store.captureScope();
		const projection = priorityProjection();

		expect(store.installSnapshot({ cursor: 10, projection }, scope)).toBe(true);
		expect(store.getSnapshot().projection).not.toBe(projection);
		expect(Object.isFrozen(store.getSnapshot().projection)).toBe(true);
		expect(
			store.installSnapshot(prioritySnapshot({ userId: OTHER_USER_ID }), scope),
		).toBe(false);
		store.reset(USER_ID, "session-b");
		expect(store.installSnapshot(prioritySnapshot(), scope)).toBe(false);
		expect(store.getSnapshot().projection).toBeNull();
	});

	it("reconciles response-before-event without republishing a duplicate", () => {
		const store = readyStore();
		const listener = vi.fn();
		store.subscribe(listener);
		const changed = priorityProjection({ revision: 2, priority: 42 });
		expect(store.beginOptimistic("response-first", 42)).toBe(true);

		expect(store.settleChanged("response-first", changed, 11)).toBe("settled");
		const settled = store.getSnapshot();
		const calls = listener.mock.calls.length;
		expect(store.applyChange({ type: "upsert", projection: changed }, 11)).toBe(
			true,
		);

		expect(listener).toHaveBeenCalledTimes(calls);
		expect(store.getSnapshot()).toBe(settled);
		expect(settled).toMatchObject({
			eventSequence: 11,
			authorityRevision: 2,
			pendingRequestIds: [],
			projection: { priority: 42 },
		});
	});

	it("keeps optimism until an event-before-response is acknowledged", () => {
		const store = readyStore();
		const changed = priorityProjection({ revision: 2, priority: 21 });
		store.beginOptimistic("event-first", 21);
		store.applyChange({ type: "upsert", projection: changed }, 11);

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			pendingRequestIds: ["event-first"],
			projection: { priority: 21 },
		});
		expect(store.settleChanged("event-first", changed, 11)).toBe("settled");
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("settles replayed no-change without cloning the tiny authority", () => {
		const store = readyStore();
		const authority = store.getSnapshot().projection;
		store.beginOptimistic("same-priority", 0);

		expect(store.getSnapshot().projection).toBe(authority);
		expect(store.settleNoChange("same-priority", priorityProjection())).toBe(
			"settled",
		);
		expect(store.getSnapshot().projection).toBe(authority);
	});

	it("rolls back one request and leaves later optimistic intent applied", () => {
		const store = readyStore();
		store.beginOptimistic("first", 10);
		store.beginOptimistic("second", 20);

		expect(store.rollback("first", new Error("rejected"))).toBe(true);
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: ["second"],
			projection: { priority: 20 },
			error: new Error("rejected"),
		});
	});

	it("tombstones pending optimism and accepts later recreation", () => {
		const store = readyStore();
		store.beginOptimistic("removed", 100);
		store.applyChange({ type: "remove", userId: USER_ID, revision: 2 }, 11);

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			authorityRevision: 2,
			projection: null,
			pendingRequestIds: [],
		});
		expect(store.settleNoChange("removed", priorityProjection())).toBe(
			"ignored",
		);

		store.applyChange(
			{
				type: "upsert",
				projection: priorityProjection({ revision: 2, priority: 7 }),
			},
			12,
		);
		expect(store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			projection: { priority: 7 },
		});
	});

	it("requires repair for divergent duplicates and revision jumps", () => {
		const store = readyStore();
		expect(() =>
			store.applyChange(
				{
					type: "upsert",
					projection: priorityProjection({ priority: 99 }),
				},
				10,
			),
		).toThrow(/sequence conflicts/);
		expect(store.getSnapshot().repairRequired).toBe(true);

		store.installRepairSnapshot(prioritySnapshot({ cursor: 12, revision: 2 }));
		expect(() =>
			store.applyChange(
				{
					type: "upsert",
					projection: priorityProjection({ revision: 4 }),
				},
				13,
			),
		).toThrow(/revision is not contiguous/);
	});
});
