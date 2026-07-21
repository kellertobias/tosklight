import { describe, expect, it } from "vitest";
import { SpeedGroupRuntimeStore } from "./store";
import {
	AUTHORITY_ID,
	changedOutcome,
	DESK_ID,
	noChangeOutcome,
	OTHER_DESK_ID,
	speedGroup,
	speedSnapshot,
} from "./testFixtures";

function readyStore() {
	const store = new SpeedGroupRuntimeStore();
	store.reset(DESK_ID, "session-a");
	store.installSnapshot(speedSnapshot());
	return store;
}

describe("SpeedGroupRuntimeStore", () => {
	it("installs exactly five ordered groups and rejects stale scopes", () => {
		const store = new SpeedGroupRuntimeStore();
		store.reset(DESK_ID, "session-a");
		const scope = store.captureScope();
		expect(store.installSnapshot(speedSnapshot(), scope)).toBe(true);
		expect(Object.isFrozen(store.getSnapshot().projection)).toBe(true);
		expect(Object.isFrozen(store.getSnapshot().projection?.groups)).toBe(true);

		store.reset(OTHER_DESK_ID, "session-b");
		expect(store.installSnapshot(speedSnapshot(), scope)).toBe(false);
		expect(store.getSnapshot().projection).toBeNull();
		expect(() =>
			store.installSnapshot(speedSnapshot({ groups: [speedGroup("A")] })),
		).toThrow(/exactly five/);
	});

	it("reconciles response-before-event and preserves broken peer authority", () => {
		const store = new SpeedGroupRuntimeStore();
		store.reset(DESK_ID, "session-a");
		store.installSnapshot(
			speedSnapshot({
				groups: [
					speedGroup("A", { synchronizedWith: "B" }),
					speedGroup("B", { synchronizedWith: "A" }),
					speedGroup("C"),
					speedGroup("D"),
					speedGroup("E"),
				],
			}),
		);
		store.beginOptimistic({
			requestId: "response-first",
			action: { type: "set_bpm", group: "A", bpm: 128 },
		});
		const groupA = speedGroup("A", {
			manualBpm: 128,
			synchronizedWith: null,
			phaseOriginMillis: 200,
		});
		const outcome = changedOutcome("response-first", [groupA]);

		expect(store.settleChanged("response-first", outcome)).toBe("settled");
		expect(
			store.getSnapshot().projection?.groups[1]?.synchronizedWith,
		).toBeNull();
		expect(
			store.applyChange(
				{
					authorityId: AUTHORITY_ID,
					revision: 2,
					appliedAtMillis: 200,
					groups: [groupA, speedGroup("B", { synchronizedWith: null })],
				},
				11,
			),
		).toBe(true);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			authorityRevision: 2,
			pendingRequestIds: [],
		});
	});

	it("keeps optimism through event-before-response and acknowledges replay", () => {
		const store = readyStore();
		store.beginOptimistic({
			requestId: "event-first",
			action: { type: "adjust_bpm", group: "A", deltaBpm: 8 },
		});
		const group = speedGroup("A", {
			manualBpm: 128,
			phaseOriginMillis: 200,
		});
		store.applyChange(
			{
				authorityId: AUTHORITY_ID,
				revision: 2,
				appliedAtMillis: 200,
				groups: [group],
			},
			11,
		);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["event-first"]);
		expect(
			store.settleChanged(
				"event-first",
				changedOutcome("event-first", [group], { replayed: true }),
			),
		).toBe("settled");
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("settles a no-change without cloning and rolls back one FIFO entry", () => {
		const store = readyStore();
		const authority = store.getSnapshot().projection;
		store.beginOptimistic({
			requestId: "same",
			action: { type: "set_bpm", group: "A", bpm: 120 },
		});
		expect(store.getSnapshot().projection).toBe(authority);
		expect(
			store.settleNoChange("same", noChangeOutcome("same", [speedGroup("A")])),
		).toBe("settled");
		expect(store.getSnapshot().projection).toBe(authority);

		store.beginOptimistic({
			requestId: "first",
			action: { type: "set_bpm", group: "A", bpm: 130 },
		});
		store.beginOptimistic({
			requestId: "second",
			action: { type: "set_bpm", group: "B", bpm: 100 },
		});
		store.rollback("first", new Error("rejected"));
		expect(store.getSnapshot().pendingRequestIds).toEqual(["second"]);
		expect(store.getSnapshot().projection?.groups[0]?.manualBpm).toBe(120);
		expect(store.getSnapshot().projection?.groups[1]?.manualBpm).toBe(100);
	});

	it("rejects invalid relative results and requires repair for revision gaps", () => {
		const store = readyStore();
		expect(
			store.beginOptimistic({
				requestId: "first-adjustment",
				action: { type: "adjust_bpm", group: "A", deltaBpm: 500 },
			}),
		).toBe(true);
		expect(() =>
			store.beginOptimistic({
				requestId: "overflow",
				action: { type: "adjust_bpm", group: "A", deltaBpm: 500 },
			}),
		).toThrow(/BPM/);
		expect(() =>
			store.applyChange(
				{
					authorityId: AUTHORITY_ID,
					revision: 3,
					appliedAtMillis: 200,
					groups: [speedGroup("A", { manualBpm: 130 })],
				},
				12,
			),
		).toThrow(/revision is not contiguous/);
		expect(store.getSnapshot().repairRequired).toBe(true);
	});
});
