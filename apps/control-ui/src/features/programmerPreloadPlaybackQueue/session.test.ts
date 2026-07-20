import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPreloadPlaybackQueueSnapshot } from "./contracts";
import { ProgrammerPreloadPlaybackQueueSession } from "./session";
import { ProgrammerPreloadPlaybackQueueStore } from "./store";
import {
	AUTHORITY_A,
	AUTHORITY_B,
	FakeProgrammerPreloadPlaybackQueueTransport,
	OTHER_USER_ID,
	queuedPlayback,
	queueSnapshot,
	SHOW_ID,
	settleQueueSession,
	USER_ID,
} from "./testFixtures";

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function harness() {
	const store = new ProgrammerPreloadPlaybackQueueStore();
	const transport = new FakeProgrammerPreloadPlaybackQueueTransport();
	const loadSnapshot = vi.fn(async () => queueSnapshot());
	const onError = vi.fn();
	const session = new ProgrammerPreloadPlaybackQueueSession({
		showId: SHOW_ID,
		userId: USER_ID,
		authorityKey: AUTHORITY_A,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { store, transport, loadSnapshot, onError, session };
}

describe("ProgrammerPreloadPlaybackQueueSession", () => {
	it("does no snapshot or socket work before its first mounted view", async () => {
		const current = harness();

		await settleQueueSession();

		expect(current.loadSnapshot).not.toHaveBeenCalled();
		expect(current.transport.subscriptions).toHaveLength(0);
	});

	it("subscribes only to the exact show and user", async () => {
		const current = harness();
		current.session.activate();
		await settleQueueSession();

		expect(current.transport.subscriptions[0]).toMatchObject({
			scope: { showId: SHOW_ID, userId: USER_ID },
			after: 10,
		});
		expect(current.store.getSnapshot()).toMatchObject({
			status: "ready",
			projection: { userId: USER_ID, revision: 2 },
		});
	});

	it("repairs a cursor gap with one narrow snapshot", async () => {
		const current = harness();
		current.session.activate();
		await settleQueueSession();
		current.loadSnapshot.mockResolvedValueOnce(
			queueSnapshot({
				cursor: 20,
				revision: 4,
				actions: [queuedPlayback({ action: "off" })],
			}),
		);
		const stream = current.transport.subscriptions[0];

		current.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settleQueueSession();

		expect(current.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(current.transport.subscriptions).toHaveLength(1);
		expect(stream.repair).toHaveBeenCalledWith(20);
		expect(current.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			repairRequired: false,
			projection: { revision: 4, actions: [{ action: "off" }] },
		});
	});

	it("rejects a foreign-user snapshot without opening a socket", async () => {
		const current = harness();
		current.loadSnapshot.mockResolvedValueOnce(
			queueSnapshot({ userId: OTHER_USER_ID }),
		);

		current.session.activate();
		await settleQueueSession();

		expect(current.transport.subscriptions).toHaveLength(0);
		expect(current.store.getSnapshot()).toMatchObject({
			projection: null,
			status: "error",
			repairRequired: true,
		});
		current.session.stop();
	});

	it("drops a late snapshot after server or session replacement", async () => {
		const current = harness();
		const stale = deferred<ProgrammerPreloadPlaybackQueueSnapshot>();
		current.loadSnapshot.mockReturnValueOnce(stale.promise);
		current.session.activate();
		await Promise.resolve();

		current.store.reset(SHOW_ID, USER_ID, AUTHORITY_B);
		const replacementTransport =
			new FakeProgrammerPreloadPlaybackQueueTransport();
		const replacement = new ProgrammerPreloadPlaybackQueueSession({
			showId: SHOW_ID,
			userId: USER_ID,
			authorityKey: AUTHORITY_B,
			store: current.store,
			transport: replacementTransport,
			loadSnapshot: async () => queueSnapshot({ cursor: 40, revision: 6 }),
		});
		replacement.activate();
		await settleQueueSession();

		stale.resolve(queueSnapshot({ cursor: 99, revision: 99 }));
		await settleQueueSession();

		expect(current.store.getSnapshot()).toMatchObject({
			authorityKey: AUTHORITY_B,
			eventSequence: 40,
			projection: { revision: 6 },
		});
		expect(current.transport.subscriptions).toHaveLength(0);
		expect(replacementTransport.subscriptions).toHaveLength(1);
	});

	it("drops a late snapshot after the active show and user change", async () => {
		const current = harness();
		const stale = deferred<ProgrammerPreloadPlaybackQueueSnapshot>();
		current.loadSnapshot.mockReturnValueOnce(stale.promise);
		current.session.activate();
		await Promise.resolve();

		current.store.reset("show-b", OTHER_USER_ID, AUTHORITY_B);
		const replacement = new ProgrammerPreloadPlaybackQueueSession({
			showId: "show-b",
			userId: OTHER_USER_ID,
			authorityKey: AUTHORITY_B,
			store: current.store,
			transport: null,
			loadSnapshot: async () =>
				queueSnapshot({
					cursor: 50,
					userId: OTHER_USER_ID,
					revision: 7,
				}),
		});
		replacement.activate();
		await settleQueueSession();

		stale.resolve(queueSnapshot({ cursor: 99, revision: 99 }));
		await settleQueueSession();

		expect(current.store.getSnapshot()).toMatchObject({
			showId: "show-b",
			userId: OTHER_USER_ID,
			authorityKey: AUTHORITY_B,
			eventSequence: 50,
			projection: { userId: OTHER_USER_ID, revision: 7 },
		});
	});

	it("drops late events after scope replacement", async () => {
		const current = harness();
		current.session.activate();
		await settleQueueSession();
		const staleStream = current.transport.subscriptions[0];

		current.store.reset(SHOW_ID, USER_ID, AUTHORITY_B);
		staleStream.observer.message({
			type: "event",
			sequence: 11,
			correlationId: null,
			projection: queueSnapshot({ revision: 3 }).projection,
		});

		expect(current.store.getSnapshot()).toMatchObject({
			authorityKey: AUTHORITY_B,
			projection: null,
		});
	});
});
