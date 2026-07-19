import { describe, expect, it, vi } from "vitest";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesProjection,
} from "./contracts";
import { ProgrammerValuesStore } from "./store";
import {
	FIXTURE_1,
	fixtureValue,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

function harness() {
	const store = new ProgrammerValuesStore();
	store.reset(SHOW_ID, USER_ID, "session-a");
	store.installSnapshot(valuesSnapshot());
	const applyAction = vi.fn<
		(
			scope: { showId: string; userId: string },
			request: ProgrammerValuesActionRequest,
		) => Promise<ProgrammerValuesActionOutcome>
	>();
	const repair = vi.fn(async () => undefined);
	const onError = vi.fn();
	const writer = new ProgrammerValuesWriter({
		scope: { showId: SHOW_ID, userId: USER_ID },
		store,
		applyAction,
		repair,
		onError,
	});
	return { store, applyAction, repair, onError, writer };
}

function changed(
	requestId: string,
	projection: ProgrammerValuesProjection,
	eventSequence = 20,
): ProgrammerValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "changed",
		revision: projection.revision,
		projection,
		eventSequence,
		replayed: false,
		warning: null,
	};
}

function noChange(requestId: string, revision = 1): ProgrammerValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change",
		revision,
		replayed: false,
		warning: null,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function fixtureInput(requestId: string, level: number) {
	return {
		requestId,
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: { kind: "normalized" as const, value: level },
		fade: false,
		fadeMillis: null,
		delayMillis: null,
	};
}

function fixtureLevel(store: ProgrammerValuesStore) {
	const value = store.getSnapshot().projection?.fixtureValues[0]?.value;
	return value?.kind === "normalized" ? value.value : null;
}

describe("ProgrammerValuesWriter reconciliation", () => {
	it("settles an HTTP response before the matching event", async () => {
		const { store, applyAction, writer } = harness();
		const projection = valuesProjection({
			revision: 2,
			fixtureValues: [fixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockResolvedValueOnce(changed("request-a", projection));

		await expect(writer.setFixtureValue(fixtureInput("request-a", 0.8))).resolves
			.toMatchObject({ status: "changed" });

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		const beforeEvent = store.getSnapshot().projection;
		store.applyProjection(projection, 20);
		expect(store.getSnapshot().projection).toBe(beforeEvent);
	});

	it("keeps intent rebased when the event arrives before HTTP", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerValuesActionOutcome>();
		const projection = valuesProjection({
			revision: 2,
			fixtureValues: [fixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("request-a", 0.8));
		await Promise.resolve();

		store.applyProjection(projection, 20);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["request-a"]);
		expect(fixtureLevel(store)).toBe(0.8);
		response.resolve(changed("request-a", projection));
		await pending;

		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(store.getSnapshot().eventSequence).toBe(20);
	});

	it("tracks and sends a predicted no-op without cloning projection", async () => {
		const { store, applyAction, writer } = harness();
		const projection = store.getSnapshot().projection;
		applyAction.mockResolvedValueOnce(noChange("same-value"));

		const pending = writer.setFixtureValue(fixtureInput("same-value", 0.25));
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["same-value"]);
		await pending;

		expect(applyAction).toHaveBeenCalledOnce();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("replays an ambiguous request once with the identical body", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(new Error("connection reset"))
			.mockResolvedValueOnce(noChange("replay-a"));

		await writer.releaseFixtureValue({
			requestId: "replay-a",
			fixtureId: "22222222-2222-4222-8222-222222222222",
			attribute: "intensity",
		});

		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
	});

	it("sends a batch as one action and one network request", async () => {
		const { applyAction, writer } = harness();
		applyAction.mockResolvedValueOnce(noChange("batch-a"));

		await writer.batch({
			requestId: "batch-a",
			mutations: [
				{
					action: "set_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					timing: { fade: false, fadeMillis: null, delayMillis: null },
				},
				{
					action: "release_group",
					groupId: "front",
					attribute: "intensity",
				},
			],
		});

		expect(applyAction).toHaveBeenCalledOnce();
		expect(applyAction.mock.calls[0]?.[1].action).toMatchObject({
			action: "batch",
			mutations: [{ action: "set_fixture" }, { action: "release_group" }],
		});
	});

	it("uses the settled revision for the next queued write", async () => {
		const { applyAction, writer } = harness();
		const first = deferred<ProgrammerValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(first.promise)
			.mockResolvedValueOnce(noChange("second", 2));
		const firstWrite = writer.setFixtureValue(fixtureInput("first", 0.8));
		const secondWrite = writer.releaseFixtureValue({
			requestId: "second",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});
		await Promise.resolve();
		first.resolve(
			changed(
				"first",
				valuesProjection({ revision: 2, fixtureValues: [fixtureValue(0.8)] }),
			),
		);
		await Promise.all([firstWrite, secondWrite]);

		expect(applyAction.mock.calls[0]?.[1].expectedRevision).toBe(1);
		expect(applyAction.mock.calls[1]?.[1].expectedRevision).toBe(2);
	});

	it("repairs a revision conflict, rolls back, and ignores late old-scope responses", async () => {
		const { store, applyAction, repair, writer } = harness();
		const conflict = Object.assign(new Error("revision conflict"), {
			status: 409,
		});
		applyAction.mockRejectedValueOnce(conflict);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(valuesSnapshot({ cursor: 12, revision: 2 }));
		});

		await writer.setFixtureValue(fixtureInput("conflict", 0.8));
		expect(repair).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
		});

		const late = deferred<ProgrammerValuesActionOutcome>();
		applyAction.mockReturnValueOnce(late.promise);
		const pending = writer.clear("late");
		await Promise.resolve();
		store.reset(SHOW_ID, USER_ID, "session-b");
		late.resolve(noChange("late", 2));
		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
		});
	});
});
