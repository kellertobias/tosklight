import { describe, expect, it, vi } from "vitest";
import type { ProgrammerValuesSnapshot } from "./contracts";
import { ProgrammerValuesSession } from "./session";
import { ProgrammerValuesStore } from "./store";
import {
	FakeProgrammerValuesTransport,
	fixtureValue,
	OTHER_USER_ID,
	settleProgrammerValuesSession,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";
import {
	type ProgrammerValuesEventTransport,
	ProgrammerValuesProtocolError,
} from "./transport";

function createHarness(
	transport: ProgrammerValuesEventTransport | null =
		new FakeProgrammerValuesTransport(),
) {
	const store = new ProgrammerValuesStore();
	const loadSnapshot = vi.fn(async () => valuesSnapshot());
	const onError = vi.fn();
	const session = new ProgrammerValuesSession({
		showId: SHOW_ID,
		userId: USER_ID,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { session, store, transport, loadSnapshot, onError };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ProgrammerValuesSession activation", () => {
	it("does no snapshot or socket work while dormant", async () => {
		const harness = createHarness();

		await settleProgrammerValuesSession();

		expect(harness.loadSnapshot).not.toHaveBeenCalled();
		expect(
			(harness.transport as FakeProgrammerValuesTransport).subscriptions,
		).toHaveLength(0);
	});

	it("reference-counts views and resumes from the cursor without a snapshot", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		const releaseFirst = harness.session.activate();
		await settleProgrammerValuesSession();

		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions[0]).toMatchObject({
			scope: { showId: SHOW_ID, userId: USER_ID },
			after: 10,
		});

		const releaseSecond = harness.session.activate();
		await settleProgrammerValuesSession();
		expect(transport.subscriptions).toHaveLength(1);
		releaseFirst();
		await settleProgrammerValuesSession();
		expect(transport.subscriptions[0].close).not.toHaveBeenCalled();

		releaseSecond();
		await settleProgrammerValuesSession();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();

		const releaseResumed = harness.session.activate();
		await settleProgrammerValuesSession();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1]).toMatchObject({ after: 10 });
		releaseResumed();
	});

	it("collapses net-zero activation churn", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		const hydration = deferred<ProgrammerValuesSnapshot>();
		harness.loadSnapshot.mockReturnValueOnce(hydration.promise);
		const release = harness.session.activate();
		await Promise.resolve();

		release();
		const releaseAgain = harness.session.activate();
		await settleProgrammerValuesSession();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();

		hydration.resolve(valuesSnapshot());
		await settleProgrammerValuesSession();
		expect(transport.subscriptions).toHaveLength(1);
		releaseAgain();
	});

	it("hydrates an active view when streaming is unavailable", async () => {
		const harness = createHarness(null);
		harness.session.activate();

		await settleProgrammerValuesSession();

		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.store.getSnapshot()).toMatchObject({
			status: "ready",
			eventSequence: 10,
			projection: { userId: USER_ID, revision: 1 },
		});
	});
});

describe("ProgrammerValuesSession authority and repair", () => {
	it("accepts only events for the scoped user", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		harness.session.activate();
		await settleProgrammerValuesSession();
		const listener = vi.fn();
		harness.store.subscribe(listener);

		transport.emit({
			type: "event",
			sequence: 20,
			correlationId: "osc-other-user",
			projection: valuesProjection({
				userId: OTHER_USER_ID,
				revision: 9,
			}),
		});

		expect(listener).not.toHaveBeenCalled();
		expect(harness.store.getSnapshot().eventSequence).toBe(10);

		transport.emit({
			type: "event",
			sequence: 21,
			correlationId: "osc-same-user",
			projection: valuesProjection({
				revision: 2,
				fixtureValues: [fixtureValue(0.75)],
			}),
		});
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 21,
			projection: { revision: 2 },
		});
	});

	it("repairs a stream gap from an authoritative snapshot", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		harness.session.activate();
		await settleProgrammerValuesSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			valuesSnapshot({
				cursor: 20,
				revision: 3,
				fixtureValues: [fixtureValue(0.7)],
			}),
		);
		const stream = transport.subscriptions[0];
		let revisionAtRepair: number | null = null;
		stream.repair.mockImplementation(() => {
			revisionAtRepair =
				harness.store.getSnapshot().projection?.revision ?? null;
		});

		transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settleProgrammerValuesSession();

		expect(revisionAtRepair).toBe(3);
		expect(stream.repair).toHaveBeenCalledWith(20);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			status: "ready",
			repairRequired: false,
			projection: { revision: 3 },
		});
	});

	it("repairs malformed transport input without opening a second stream", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		harness.session.activate();
		await settleProgrammerValuesSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			valuesSnapshot({ cursor: 30, revision: 4 }),
		);
		const malformed = new ProgrammerValuesProtocolError(
			"Malformed Programmer values event",
			29,
		);

		transport.subscriptions[0].observer.error(malformed);
		await settleProgrammerValuesSession();

		expect(harness.onError).toHaveBeenCalledWith(malformed);
		expect(harness.onError).toHaveBeenLastCalledWith(null);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].repair).toHaveBeenCalledWith(30);
	});

	it("replaces same-revision divergence during protocol repair", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammerValuesTransport;
		harness.session.activate();
		await settleProgrammerValuesSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			valuesSnapshot({
				cursor: 15,
				revision: 1,
				fixtureValues: [fixtureValue(0.9)],
			}),
		);

		transport.emit({
			type: "event",
			sequence: 14,
			correlationId: null,
			projection: valuesProjection({
				revision: 1,
				fixtureValues: [fixtureValue(0.8)],
			}),
		});
		await settleProgrammerValuesSession();

		const value = harness.store.getSnapshot().projection?.fixtureValues[0]?.value;
		expect(value).toEqual({ kind: "normalized", value: 0.9 });
		expect(transport.subscriptions[0].repair).toHaveBeenCalledWith(15);
		expect(harness.store.getSnapshot().repairRequired).toBe(false);
	});
});
