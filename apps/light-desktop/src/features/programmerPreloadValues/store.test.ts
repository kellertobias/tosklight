import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPreloadValuesProjection } from "./contracts";
import { ProgrammerPreloadValuesStore } from "./store";
import {
	FIXTURE_1,
	FIXTURE_2,
	OTHER_USER_ID,
	preloadFixtureValue,
	preloadProjection,
	preloadSnapshot,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";
import { ProgrammerPreloadValuesProtocolError } from "./transport";

function readyStore(projection = preloadProjection()) {
	const store = new ProgrammerPreloadValuesStore();
	store.reset(SHOW_ID, USER_ID, "session-a");
	store.installSnapshot({ cursor: 10, projection });
	return store;
}

function setFixtureLevel(level: number) {
	return (
		current: ProgrammerPreloadValuesProjection,
	): ProgrammerPreloadValuesProjection => ({
		...current,
		fixtureValues: current.fixtureValues.map((entry) =>
			entry.fixtureId === FIXTURE_1
				? { ...entry, value: { kind: "normalized", value: level } }
				: entry,
		),
	});
}

function fixtureLevel(store: ProgrammerPreloadValuesStore) {
	const value = store.getSnapshot().projection?.fixtureValues[0]?.value;
	return value?.kind === "normalized" ? value.value : null;
}

describe("ProgrammerPreloadValuesStore authority", () => {
	it("accepts only the exact user and invalidates all work on scope replacement", () => {
		const store = readyStore();
		const oldScope = store.captureScope();
		store.beginOptimistic("pending", setFixtureLevel(0.8), oldScope);

		expect(
			store.applyProjection(
				preloadProjection({ userId: OTHER_USER_ID, revision: 2 }),
				11,
			),
		).toBe(false);
		store.reset(SHOW_ID, USER_ID, "session-b");

		expect(store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
			status: "idle",
		});
		expect(
			store.installSnapshot(preloadSnapshot(), { expectedScope: oldScope }),
		).toBe(false);
	});

	it("canonicalizes Programmer order, timing, and immutable value payloads", () => {
		const store = readyStore(
			preloadProjection({
				fixtureValues: [
					preloadFixtureValue(0.7, {
						fixtureId: FIXTURE_2,
						programmerOrder: 4,
						fade: true,
						fadeMillis: 300,
						delayMillis: 50,
						value: { kind: "spread", value: [0.1, 0.9] },
					}),
					preloadFixtureValue(0.2, { programmerOrder: 1 }),
				],
			}),
		);
		const projection = store.getSnapshot().projection;

		expect(projection?.fixtureValues.map(({ fixtureId }) => fixtureId)).toEqual(
			[FIXTURE_1, FIXTURE_2],
		);
		expect(projection?.fixtureValues[1]).toMatchObject({
			fade: true,
			fadeMillis: 300,
			delayMillis: 50,
		});
		expect(Object.isFrozen(projection)).toBe(true);
		const spread = projection?.fixtureValues[1]?.value;
		expect(spread?.kind === "spread" && Object.isFrozen(spread.value)).toBe(
			true,
		);
	});

	it("rejects duplicate addresses as a repairable protocol error", () => {
		const store = new ProgrammerPreloadValuesStore();
		store.reset(SHOW_ID, USER_ID);

		expect(() =>
			store.installSnapshot(
				preloadSnapshot({
					fixtureValues: [preloadFixtureValue(), preloadFixtureValue(0.8)],
				}),
			),
		).toThrow(ProgrammerPreloadValuesProtocolError);
		expect(store.getSnapshot()).toMatchObject({
			status: "error",
			repairRequired: true,
			projection: null,
		});
	});
});

describe("ProgrammerPreloadValuesStore reconciliation", () => {
	it("rebases optimism over an event and rolls back to that authority", () => {
		const store = readyStore();
		store.beginOptimistic("write-a", setFixtureLevel(0.8));

		store.applyProjection(
			preloadProjection({
				revision: 2,
				fixtureValues: [preloadFixtureValue(0.4)],
			}),
			20,
		);
		expect(fixtureLevel(store)).toBe(0.8);
		store.rollback("write-a", new Error("rejected"));

		expect(fixtureLevel(store)).toBe(0.4);
		expect(store.getSnapshot().eventSequence).toBe(20);
	});

	it("settles response-first and ignores the identical later event", () => {
		const store = readyStore();
		const projection = preloadProjection({
			revision: 2,
			fixtureValues: [preloadFixtureValue(0.8)],
		});
		store.beginOptimistic("write-a", setFixtureLevel(0.8));
		expect(store.settleChanged("write-a", projection, 20)).toBe("settled");
		const settled = store.getSnapshot().projection;
		const listener = vi.fn();
		store.subscribe(listener);

		store.applyProjection(projection, 20);

		expect(store.getSnapshot().projection).toBe(settled);
		expect(listener).not.toHaveBeenCalled();
	});

	it("keeps no-op projection identity while tracking and settling the request", () => {
		const store = readyStore();
		const projection = store.getSnapshot().projection;
		store.beginOptimistic("no-op", (current) => current);

		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.settleNoChange("no-op", 1)).toBe("settled");
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("requires repair for divergent authority at one revision or cursor", () => {
		const store = readyStore();

		expect(() =>
			store.applyProjection(
				preloadProjection({
					revision: 1,
					fixtureValues: [preloadFixtureValue(0.9)],
				}),
				10,
			),
		).toThrow(ProgrammerPreloadValuesProtocolError);
		expect(store.getSnapshot().repairRequired).toBe(true);
	});

	it("repairs instead of consuming a newer cursor with an older revision", () => {
		const store = readyStore(preloadProjection({ revision: 2 }));

		expect(() =>
			store.applyProjection(preloadProjection({ revision: 1 }), 11),
		).toThrow(ProgrammerPreloadValuesProtocolError);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 10,
			projection: { revision: 2 },
			repairRequired: true,
		});
	});
});
