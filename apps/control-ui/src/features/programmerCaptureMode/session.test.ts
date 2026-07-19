import { describe, expect, it, vi } from "vitest";
import type { ProgrammerCaptureModeSnapshot } from "./contracts";
import { ProgrammerCaptureModeSession } from "./session";
import { ProgrammerCaptureModeStore } from "./store";
import {
	captureModeProjection,
	captureModeSnapshot,
	FakeProgrammerCaptureModeTransport,
	OTHER_USER_ID,
	SHOW_ID,
	settleCaptureModeSession,
	USER_ID,
} from "./testFixtures";

function createHarness() {
	const store = new ProgrammerCaptureModeStore();
	const transport = new FakeProgrammerCaptureModeTransport();
	const loadSnapshot = vi.fn(async () => captureModeSnapshot());
	const onError = vi.fn();
	const session = new ProgrammerCaptureModeSession({
		showId: SHOW_ID,
		userId: USER_ID,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { store, transport, loadSnapshot, onError, session };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ProgrammerCaptureModeSession", () => {
	it("stays dormant, shares activation, and subscribes to the exact scope", async () => {
		const harness = createHarness();
		await settleCaptureModeSession();
		expect(harness.loadSnapshot).not.toHaveBeenCalled();
		expect(harness.transport.subscriptions).toHaveLength(0);

		const releaseFirst = harness.session.activate();
		const releaseSecond = harness.session.activate();
		await settleCaptureModeSession();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.transport.subscriptions).toHaveLength(1);
		expect(harness.transport.subscriptions[0]).toMatchObject({
			scope: { showId: SHOW_ID, userId: USER_ID },
			after: 10,
		});

		releaseFirst();
		await settleCaptureModeSession();
		expect(harness.transport.subscriptions[0].close).not.toHaveBeenCalled();
		releaseSecond();
		await settleCaptureModeSession();
		expect(harness.transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("applies events and repairs a gap from one authoritative snapshot", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleCaptureModeSession();
		harness.transport.emit({
			type: "event",
			sequence: 11,
			correlationId: "osc-mode",
			projection: captureModeProjection({ revision: 2, blind: true }),
		});
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 11,
			projection: { revision: 2, blind: true },
		});

		harness.loadSnapshot.mockResolvedValueOnce(
			captureModeSnapshot({
				cursor: 20,
				revision: 3,
				blind: true,
				preloadCaptureProgrammer: true,
			}),
		);
		harness.transport.emit({
			type: "gap",
			afterSequence: 11,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settleCaptureModeSession();

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.transport.subscriptions[0].repair).toHaveBeenCalledWith(20);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			repairRequired: false,
			projection: { revision: 3, preloadCaptureProgrammer: true },
		});
	});

	it("joins a writer repair to an in-flight gap repair", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleCaptureModeSession();
		const repairSnapshot = deferred<ProgrammerCaptureModeSnapshot>();
		harness.loadSnapshot.mockReturnValueOnce(repairSnapshot.promise);

		harness.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await Promise.resolve();
		let writerRepairSettled = false;
		const writerRepair = harness.session
			.repairAuthority(new Error("revision conflict"))
			.then(() => {
				writerRepairSettled = true;
			});

		await Promise.resolve();
		expect(writerRepairSettled).toBe(false);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		repairSnapshot.resolve(
			captureModeSnapshot({ cursor: 20, revision: 3, preview: true }),
		);
		await writerRepair;

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			repairRequired: false,
			projection: { revision: 3, preview: true },
		});
	});

	it("rejects a foreign event and repairs before accepting more events", async () => {
		const harness = createHarness();
		harness.session.activate();
		await settleCaptureModeSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			captureModeSnapshot({ cursor: 20 }),
		);

		harness.transport.emit({
			type: "event",
			sequence: 19,
			correlationId: "foreign",
			projection: captureModeProjection({
				userId: OTHER_USER_ID,
				revision: 9,
			}),
		});
		await settleCaptureModeSession();

		expect(harness.onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: expect.stringContaining("event user"),
			}),
		);
		expect(harness.transport.subscriptions[0].repair).toHaveBeenCalledWith(20);
		expect(harness.store.getSnapshot().projection?.userId).toBe(USER_ID);

		harness.transport.emit({
			type: "event",
			sequence: 21,
			correlationId: "same-user",
			projection: captureModeProjection({ revision: 2, preview: true }),
		});
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 21,
			projection: { revision: 2, preview: true },
		});
	});

	it("cannot install a late snapshot after the session scope is replaced", async () => {
		const store = new ProgrammerCaptureModeStore();
		const pending = deferred<ProgrammerCaptureModeSnapshot>();
		const session = new ProgrammerCaptureModeSession({
			showId: SHOW_ID,
			userId: USER_ID,
			store,
			transport: null,
			loadSnapshot: () => pending.promise,
		});
		session.activate();
		await Promise.resolve();

		session.stop();
		store.reset("replacement-show", OTHER_USER_ID, "server-b");
		pending.resolve(captureModeSnapshot({ revision: 99 }));
		await settleCaptureModeSession();

		expect(store.getSnapshot()).toMatchObject({
			showId: "replacement-show",
			userId: OTHER_USER_ID,
			projection: null,
		});
	});

	it("repairs authority after the last active view closes its stream", async () => {
		const harness = createHarness();
		const release = harness.session.activate();
		await settleCaptureModeSession();
		release();
		await settleCaptureModeSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			captureModeSnapshot({ cursor: 30, revision: 4, blind: true }),
		);

		await harness.session.repairAuthority(new Error("revision conflict"));

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.transport.subscriptions).toHaveLength(1);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 30,
			repairRequired: false,
			projection: { revision: 4, blind: true },
		});
	});

	it("reports capture authority as unavailable before any view mounts", async () => {
		const harness = createHarness();

		await expect(
			harness.session.repairAuthority(new Error("revision conflict")),
		).rejects.toThrow("Programmer capture mode session is unavailable");
		expect(harness.loadSnapshot).not.toHaveBeenCalled();
	});
});
