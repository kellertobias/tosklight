import { describe, expect, it } from "vitest";
import { ProgrammerLifecycleStore } from "./store";
import {
	AUTHORITY_A,
	lifecycleRow,
	lifecycleSnapshot,
	otherLifecycleRow,
	PROGRAMMER_A,
	removalChange,
	upsertChange,
} from "./testFixtures";

function hydratedStore() {
	const store = new ProgrammerLifecycleStore();
	store.reset(AUTHORITY_A);
	store.installSnapshot(lifecycleSnapshot());
	return store;
}

describe("ProgrammerLifecycleStore deltas", () => {
	it("applies one ordered upsert and removal per contiguous revision", () => {
		const store = hydratedStore();
		const unchanged = store.getSnapshot().projection?.programmers[0];

		store.applyChange(upsertChange(otherLifecycleRow(), 5), 11);
		expect(store.getSnapshot().projection?.programmers[0]).toBe(unchanged);
		expect(store.getSnapshot().projection).toMatchObject({
			revision: 5,
			programmers: [{ userId: "operator-a" }, { userId: "operator-b" }],
		});

		store.applyChange(removalChange(PROGRAMMER_A, 6), 12);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 12,
			projection: {
				revision: 6,
				programmers: [{ userId: "operator-b" }],
			},
		});
	});

	it("canonicalizes row and session order", () => {
		const store = new ProgrammerLifecycleStore();
		store.reset(AUTHORITY_A);
		store.installSnapshot(
			lifecycleSnapshot({
				programmers: [
					otherLifecycleRow(),
					lifecycleRow({
						sessions: [{ sessionId: "z" }, { sessionId: "a" }],
					}),
				],
			}),
		);

		const projection = store.getSnapshot().projection;
		expect(projection?.programmers.map((row) => row.userId)).toEqual([
			"operator-a",
			"operator-b",
		]);
		expect(
			projection?.programmers[0].sessions.map((row) => row.sessionId),
		).toEqual(["a", "z"]);
	});

	it("replaces the prior Programmer identity for the same user", () => {
		const store = hydratedStore();

		store.applyChange(
			upsertChange(
				lifecycleRow({
					programmerId: "33333333-3333-4333-8333-333333333333",
					normalValueCount: 0,
				}),
				5,
			),
			11,
		);

		expect(store.getSnapshot().projection).toMatchObject({
			revision: 5,
			programmers: [
				{
					programmerId: "33333333-3333-4333-8333-333333333333",
					userId: "operator-a",
				},
			],
		});
	});

	it("accepts filtered cursor jumps but rejects revision gaps", () => {
		const filteredCursor = hydratedStore();
		filteredCursor.applyChange(upsertChange(otherLifecycleRow(), 5), 42);
		expect(filteredCursor.getSnapshot()).toMatchObject({
			eventSequence: 42,
			projection: { revision: 5 },
		});
		filteredCursor.applyChange(removalChange(PROGRAMMER_A, 6), 20);
		expect(filteredCursor.getSnapshot()).toMatchObject({
			eventSequence: 42,
			projection: { revision: 5 },
		});

		const revisionGap = hydratedStore();
		expect(() =>
			revisionGap.applyChange(upsertChange(otherLifecycleRow(), 7), 11),
		).toThrow(/revision is not contiguous/);
		expect(revisionGap.getSnapshot().repairRequired).toBe(true);
	});
});
