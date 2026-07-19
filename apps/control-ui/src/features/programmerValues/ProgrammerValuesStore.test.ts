import { describe, expect, it, vi } from "vitest";
import type {
	ProgrammerFixtureValue,
	ProgrammerValuesProjection,
} from "./contracts";
import { ProgrammerValuesStore } from "./store";
import {
	FIXTURE_1,
	FIXTURE_2,
	fixtureValue,
	groupValue,
	OTHER_SHOW_ID,
	OTHER_USER_ID,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";
import { ProgrammerValuesProtocolError } from "./transport";

function readyStore(projection = valuesProjection()) {
	const store = new ProgrammerValuesStore();
	store.reset(SHOW_ID, USER_ID);
	store.installSnapshot({ cursor: 10, projection });
	return store;
}

function setFixtureLevel(level: number) {
	return (current: ProgrammerValuesProjection): ProgrammerValuesProjection => ({
		...current,
		fixtureValues: current.fixtureValues.map((entry) =>
			entry.fixtureId === FIXTURE_1 && entry.attribute === "intensity"
				? { ...entry, value: { kind: "normalized", value: level } }
				: entry,
		),
	});
}

function addToFixtureLevel(delta: number) {
	return (current: ProgrammerValuesProjection): ProgrammerValuesProjection => ({
		...current,
		fixtureValues: current.fixtureValues.map((entry) => {
			if (entry.fixtureId !== FIXTURE_1 || entry.value.kind !== "normalized")
				return entry;
			return {
				...entry,
				value: { kind: "normalized", value: entry.value.value + delta },
			};
		}),
	});
}

function fixtureLevel(store: ProgrammerValuesStore) {
	const value = store.getSnapshot().projection?.fixtureValues[0]?.value;
	return value?.kind === "normalized" ? value.value : null;
}

describe("ProgrammerValuesStore authority", () => {
	it("isolates all authority and pending work by show and user", () => {
		const store = readyStore();
		const oldScope = store.captureScope();
		expect(store.beginOptimistic("request-a", setFixtureLevel(0.8))).toBe(true);

		store.reset(OTHER_SHOW_ID, OTHER_USER_ID);

		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			userId: OTHER_USER_ID,
			projection: null,
			pendingRequestIds: [],
			status: "idle",
		});
		expect(store.installSnapshot(valuesSnapshot())).toBe(false);
		expect(
			store.installSnapshot(
				valuesSnapshot({ userId: OTHER_USER_ID }),
				{ expectedScope: oldScope },
			),
		).toBe(false);
	});

	it("ignores authority for another user in the same show", () => {
		const store = readyStore();
		const listener = vi.fn();
		store.subscribe(listener);

		expect(
			store.applyProjection(
				valuesProjection({ userId: OTHER_USER_ID, revision: 9 }),
				20,
			),
		).toBe(false);
		expect(listener).not.toHaveBeenCalled();
		expect(store.getSnapshot().eventSequence).toBe(10);
	});

	it("canonicalizes order and publishes deeply immutable views", () => {
		const store = readyStore(
			valuesProjection({
				fixtureValues: [
					fixtureValue(0.7, {
						fixtureId: FIXTURE_2,
						programmerOrder: 3,
						value: { kind: "spread", value: [0.1, 0.9] },
					}),
					fixtureValue(0.2, { programmerOrder: 1 }),
				],
				groupValues: [groupValue(0.4)],
			}),
		);
		const projection = store.getSnapshot().projection;

		expect(projection?.fixtureValues.map(({ fixtureId }) => fixtureId)).toEqual([
			FIXTURE_1,
			FIXTURE_2,
		]);
		expect(Object.isFrozen(projection)).toBe(true);
		expect(Object.isFrozen(projection?.fixtureValues)).toBe(true);
		expect(Object.isFrozen(projection?.fixtureValues[1]?.value)).toBe(true);
		const spread = projection?.fixtureValues[1]?.value;
		expect(spread?.kind === "spread" && Object.isFrozen(spread.value)).toBe(true);
	});

	it("rejects duplicate addresses as a repairable protocol error", () => {
		const store = new ProgrammerValuesStore();
		store.reset(SHOW_ID, USER_ID);

		expect(() =>
			store.installSnapshot(
				valuesSnapshot({
					fixtureValues: [fixtureValue(), fixtureValue(0.8)],
				}),
			),
		).toThrow(ProgrammerValuesProtocolError);
		expect(store.getSnapshot()).toMatchObject({
			status: "error",
			repairRequired: true,
			projection: null,
		});
	});
});

describe("ProgrammerValuesStore optimism", () => {
	it("publishes an optimistic value immediately", () => {
		const store = readyStore();

		expect(store.beginOptimistic("software-1", setFixtureLevel(0.8))).toBe(
			true,
		);
		expect(fixtureLevel(store)).toBe(0.8);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["software-1"]);
		expect(Object.isFrozen(store.getSnapshot().pendingRequestIds)).toBe(true);
	});

	it("rebases pending local intent over newer external authority", () => {
		const store = readyStore();
		store.beginOptimistic("software-1", setFixtureLevel(0.8));

		store.applyProjection(
			valuesProjection({
				revision: 2,
				fixtureValues: [fixtureValue(0.4)],
				groupValues: [groupValue(0.6)],
			}),
			18,
		);

		expect(fixtureLevel(store)).toBe(0.8);
		expect(store.getSnapshot().projection).toMatchObject({
			revision: 2,
			groupValues: [{ groupId: "front" }],
		});
		store.rollback("software-1", new Error("write rejected"));
		expect(fixtureLevel(store)).toBe(0.4);
		expect(store.getSnapshot().eventSequence).toBe(18);
	});

	it("rolls back one operation and recomputes later reducers", () => {
		const store = readyStore();
		store.beginOptimistic("first", setFixtureLevel(0.8));
		store.beginOptimistic("second", addToFixtureLevel(0.1));
		expect(fixtureLevel(store)).toBeCloseTo(0.9);

		store.rollback("first", new Error("first failed"));

		expect(fixtureLevel(store)).toBeCloseTo(0.35);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["second"]);
		store.commit("second");
		expect(fixtureLevel(store)).toBe(0.25);
	});

	it("installs a response before replaying remaining operations", () => {
		const store = readyStore();
		store.beginOptimistic("first", setFixtureLevel(0.5));
		store.beginOptimistic("second", addToFixtureLevel(0.1));

		store.commit(
			"first",
			valuesProjection({ revision: 2, fixtureValues: [fixtureValue(0.5)] }),
		);

		expect(fixtureLevel(store)).toBeCloseTo(0.6);
		expect(store.getSnapshot().projection?.revision).toBe(2);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["second"]);
	});

	it("does not let a late operation cross a user scope reset", () => {
		const store = readyStore();
		const scope = store.captureScope();
		store.beginOptimistic("request-a", setFixtureLevel(0.8), scope);
		store.reset(SHOW_ID, OTHER_USER_ID);

		expect(
			store.commit(
				"request-a",
				valuesProjection({ userId: USER_ID, revision: 2 }),
				scope,
			),
		).toBe(false);
	});

	it("invalidates authority when the server session changes in-place", () => {
		const store = new ProgrammerValuesStore();
		store.reset(SHOW_ID, USER_ID, "session-a");
		store.installSnapshot(valuesSnapshot());
		const oldScope = store.captureScope();

		store.reset(SHOW_ID, USER_ID, "session-b");

		expect(store.getSnapshot()).toMatchObject({
			showId: SHOW_ID,
			userId: USER_ID,
			projection: null,
			status: "idle",
		});
		expect(store.isScopeCurrent(oldScope)).toBe(false);
	});
});

describe("ProgrammerValuesStore revision and cursor ordering", () => {
	it("ignores stale cursors and projections while advancing valid cursors", () => {
		const store = readyStore(valuesProjection({ revision: 2 }));
		store.applyProjection(
			valuesProjection({ revision: 99, fixtureValues: [fixtureValue(0.99)] }),
			9,
		);
		expect(fixtureLevel(store)).toBe(0.25);
		expect(store.getSnapshot().eventSequence).toBe(10);

		store.applyProjection(
			valuesProjection({ revision: 1, fixtureValues: [fixtureValue(0.1)] }),
			11,
		);
		expect(fixtureLevel(store)).toBe(0.25);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			projection: { revision: 2 },
		});
	});

	it("does not notify projection selectors for an exact duplicate cursor", () => {
		const projection = valuesProjection({ revision: 2 });
		const store = readyStore(projection);
		const listener = vi.fn();
		store.subscribe(listener);

		store.applyProjection(projection, 10);

		expect(listener).not.toHaveBeenCalled();
	});

	it("rejects same-revision divergence atomically and requests repair", () => {
		const store = readyStore();

		expect(() =>
			store.applyProjection(
				valuesProjection({ revision: 1, fixtureValues: [fixtureValue(0.9)] }),
				11,
			),
		).toThrow(ProgrammerValuesProtocolError);
		expect(fixtureLevel(store)).toBe(0.25);
		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 10,
			status: "error",
			repairRequired: true,
		});
	});

	it("rejects a different event at the same cursor", () => {
		const store = readyStore();
		const conflicting: ProgrammerFixtureValue = fixtureValue(0.8);

		expect(() =>
			store.applyProjection(
				valuesProjection({ revision: 2, fixtureValues: [conflicting] }),
				10,
			),
		).toThrow(ProgrammerValuesProtocolError);
		expect(fixtureLevel(store)).toBe(0.25);
	});
});
