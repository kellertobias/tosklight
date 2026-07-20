import { describe, expect, it, vi } from "vitest";
import type { ProgrammerLifecycleSnapshot } from "./contracts";
import { ProgrammerLifecycleSession } from "./session";
import { ProgrammerLifecycleStore } from "./store";
import {
	AUTHORITY_A,
	AUTHORITY_B,
	FakeProgrammerLifecycleTransport,
	lifecycleSnapshot,
	otherLifecycleRow,
	PROGRAMMER_A,
	removalChange,
	settleLifecycleSession,
	upsertChange,
} from "./testFixtures";

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

function createHarness() {
	const store = new ProgrammerLifecycleStore();
	const transport = new FakeProgrammerLifecycleTransport();
	const loadSnapshot = vi.fn(async () => lifecycleSnapshot());
	const onError = vi.fn();
	const session = new ProgrammerLifecycleSession({
		authorityKey: AUTHORITY_A,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { store, transport, loadSnapshot, onError, session };
}

describe("ProgrammerLifecycleSession activation", () => {
	it("stays dormant and reference-counts mounted views", async () => {
		const harness = createHarness();

		await settleLifecycleSession();
		expect(harness.loadSnapshot).not.toHaveBeenCalled();
		expect(harness.transport.subscriptions).toHaveLength(0);

		const releaseFirst = harness.session.activate();
		await settleLifecycleSession();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.transport.subscriptions[0]).toMatchObject({ after: 10 });

		const releaseSecond = harness.session.activate();
		releaseFirst();
		await settleLifecycleSession();
		expect(harness.transport.subscriptions[0].close).not.toHaveBeenCalled();

		releaseSecond();
		await settleLifecycleSession();
		expect(harness.transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("reuses the scoped projection when its session object is replaced", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleLifecycleSession();
		harness.session.stop();
		const replacementTransport = new FakeProgrammerLifecycleTransport();
		const replacementLoad = vi.fn(async () => lifecycleSnapshot());
		const replacement = new ProgrammerLifecycleSession({
			authorityKey: AUTHORITY_A,
			store: harness.store,
			transport: replacementTransport,
			loadSnapshot: replacementLoad,
		});

		replacement.activate();
		await settleLifecycleSession();

		expect(replacementLoad).not.toHaveBeenCalled();
		expect(replacementTransport.subscriptions[0]).toMatchObject({ after: 10 });
	});
});

describe("ProgrammerLifecycleSession authority", () => {
	it("applies contiguous row upserts and removals", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleLifecycleSession();

		harness.transport.emit({
			type: "event",
			sequence: 11,
			correlationId: "same-user-peer",
			change: upsertChange(otherLifecycleRow(), 5),
		});
		harness.transport.emit({
			type: "event",
			sequence: 12,
			correlationId: "disconnect",
			change: removalChange(PROGRAMMER_A, 6),
		});

		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 12,
			projection: {
				revision: 6,
				programmers: [{ userId: "operator-b" }],
			},
		});
	});

	it("repairs a reported cursor gap with one narrow snapshot", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleLifecycleSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			lifecycleSnapshot({
				cursor: 20,
				revision: 7,
				programmers: [otherLifecycleRow({ normalValueCount: 9 })],
			}),
		);
		const stream = harness.transport.subscriptions[0];

		harness.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settleLifecycleSession();

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.transport.subscriptions).toHaveLength(1);
		expect(stream.repair).toHaveBeenCalledWith(20);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			repairRequired: false,
			projection: {
				revision: 7,
				programmers: [{ normalValueCount: 9 }],
			},
		});
	});

	it("ignores a late load after authority replacement", async () => {
		const store = new ProgrammerLifecycleStore();
		const staleLoad = deferred<ProgrammerLifecycleSnapshot>();
		const staleTransport = new FakeProgrammerLifecycleTransport();
		const stale = new ProgrammerLifecycleSession({
			authorityKey: AUTHORITY_A,
			store,
			transport: staleTransport,
			loadSnapshot: () => staleLoad.promise,
		});
		stale.activate();
		await Promise.resolve();

		store.reset(AUTHORITY_B);
		const currentTransport = new FakeProgrammerLifecycleTransport();
		const current = new ProgrammerLifecycleSession({
			authorityKey: AUTHORITY_B,
			store,
			transport: currentTransport,
			loadSnapshot: async () =>
				lifecycleSnapshot({ cursor: 40, revision: 2, programmers: [] }),
		});
		current.activate();
		await settleLifecycleSession();

		staleLoad.resolve(lifecycleSnapshot({ cursor: 99, revision: 99 }));
		await settleLifecycleSession();
		expect(store.getSnapshot()).toMatchObject({
			authorityKey: AUTHORITY_B,
			eventSequence: 40,
			projection: { revision: 2, programmers: [] },
		});
		expect(staleTransport.subscriptions).toHaveLength(0);
		expect(currentTransport.subscriptions).toHaveLength(1);
	});

	it("ignores a late load after disposal", async () => {
		const harness = createHarness();
		const load = deferred<ProgrammerLifecycleSnapshot>();
		harness.loadSnapshot.mockReturnValueOnce(load.promise);
		harness.session.activate();
		await Promise.resolve();
		harness.session.stop();

		load.resolve(lifecycleSnapshot({ cursor: 30, revision: 8 }));
		await settleLifecycleSession();
		expect(harness.transport.subscriptions).toHaveLength(0);
		expect(harness.store.getSnapshot()).toMatchObject({
			projection: null,
			status: "loading",
		});
	});

	it("ignores late events after authority replacement or disposal", async () => {
		const replaced = createHarness();
		replaced.session.activate();
		await settleLifecycleSession();
		const replacedStream = replaced.transport.subscriptions[0];
		replaced.store.reset(AUTHORITY_B);
		replacedStream.observer.message({
			type: "event",
			sequence: 11,
			correlationId: null,
			change: upsertChange(otherLifecycleRow(), 5),
		});
		expect(replaced.store.getSnapshot()).toMatchObject({
			authorityKey: AUTHORITY_B,
			projection: null,
		});

		const disposed = createHarness();
		disposed.session.activate();
		await settleLifecycleSession();
		const disposedStream = disposed.transport.subscriptions[0];
		disposed.session.stop();
		disposedStream.observer.message({
			type: "event",
			sequence: 11,
			correlationId: null,
			change: upsertChange(otherLifecycleRow(), 5),
		});
		expect(disposed.store.getSnapshot()).toMatchObject({
			eventSequence: 10,
			projection: { revision: 4 },
		});
	});
});
