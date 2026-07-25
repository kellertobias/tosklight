import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPreloadValuesSnapshot } from "./contracts";
import { ProgrammerPreloadValuesSession } from "./session";
import { ProgrammerPreloadValuesStore } from "./store";
import {
	FakeProgrammerPreloadValuesTransport,
	OTHER_USER_ID,
	preloadFixtureValue,
	preloadProjection,
	preloadSnapshot,
	SHOW_ID,
	settlePreloadSession,
	USER_ID,
} from "./testFixtures";

function harness() {
	const store = new ProgrammerPreloadValuesStore();
	const transport = new FakeProgrammerPreloadValuesTransport();
	const loadSnapshot = vi.fn(async () => preloadSnapshot());
	const onError = vi.fn();
	const session = new ProgrammerPreloadValuesSession({
		showId: SHOW_ID,
		userId: USER_ID,
		authorityKey: "session-a",
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { store, transport, loadSnapshot, onError, session };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

describe("ProgrammerPreloadValuesSession", () => {
	it("does no snapshot or socket work before its first mounted view", async () => {
		const current = harness();

		await settlePreloadSession();

		expect(current.loadSnapshot).not.toHaveBeenCalled();
		expect(current.transport.subscriptions).toHaveLength(0);
	});

	it("hydrates and subscribes only to the exact current user", async () => {
		const current = harness();
		const release = current.session.activate();
		await settlePreloadSession();

		expect(current.loadSnapshot).toHaveBeenCalledOnce();
		expect(current.transport.subscriptions[0]).toMatchObject({
			scope: { showId: SHOW_ID, userId: USER_ID },
			after: 10,
		});
		expect(current.store.getSnapshot()).toMatchObject({
			status: "ready",
			projection: { userId: USER_ID, revision: 1 },
		});

		release();
		await settlePreloadSession();
		expect(current.transport.subscriptions[0]?.close).toHaveBeenCalledOnce();
	});

	it("repairs a gap in-place from one authoritative snapshot", async () => {
		const current = harness();
		current.session.activate();
		await settlePreloadSession();
		current.loadSnapshot.mockResolvedValueOnce(
			preloadSnapshot({
				cursor: 20,
				revision: 3,
				fixtureValues: [preloadFixtureValue(0.7)],
			}),
		);

		current.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settlePreloadSession();

		expect(current.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(current.transport.subscriptions).toHaveLength(1);
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);
		expect(current.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			status: "ready",
			repairRequired: false,
			projection: { revision: 3 },
		});
	});

	it("rejects a foreign-user event and repairs exact-user authority", async () => {
		const current = harness();
		current.session.activate();
		await settlePreloadSession();
		current.loadSnapshot.mockResolvedValueOnce(
			preloadSnapshot({ cursor: 20, revision: 2 }),
		);

		current.transport.emit({
			type: "event",
			sequence: 19,
			correlationId: "foreign",
			projection: preloadProjection({
				userId: OTHER_USER_ID,
				revision: 9,
			}),
		});
		await settlePreloadSession();

		expect(current.onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: expect.stringContaining("event user"),
			}),
		);
		expect(current.store.getSnapshot()).toMatchObject({
			projection: { userId: USER_ID, revision: 2 },
			repairRequired: false,
		});
	});

	it("rejects a foreign-user snapshot without opening a socket", async () => {
		const current = harness();
		current.loadSnapshot.mockResolvedValueOnce(
			preloadSnapshot({ userId: OTHER_USER_ID }),
		);

		current.session.activate();
		await settlePreloadSession();

		expect(current.transport.subscriptions).toHaveLength(0);
		expect(current.store.getSnapshot()).toMatchObject({
			projection: null,
			status: "error",
		});
		expect(current.onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: expect.stringContaining("snapshot user"),
			}),
		);
		current.session.stop();
	});

	it("drops a late snapshot after server/session scope replacement", async () => {
		const current = harness();
		const snapshot = deferred<ProgrammerPreloadValuesSnapshot>();
		current.loadSnapshot.mockReturnValueOnce(snapshot.promise);
		current.session.activate();
		await Promise.resolve();

		current.store.reset(SHOW_ID, USER_ID, "session-b");
		snapshot.resolve(preloadSnapshot({ revision: 9 }));
		await settlePreloadSession();

		expect(current.store.getSnapshot()).toMatchObject({
			projection: null,
			status: "idle",
		});
		expect(current.transport.subscriptions).toHaveLength(0);
	});
});
