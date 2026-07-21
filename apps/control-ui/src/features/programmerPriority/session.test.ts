import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPrioritySnapshot } from "./contracts";
import { ProgrammerPrioritySession } from "./session";
import { ProgrammerPriorityStore } from "./store";
import {
	deferred,
	FakeProgrammerPriorityTransport,
	OTHER_USER_ID,
	priorityProjection,
	prioritySnapshot,
	settlePrioritySession,
	USER_ID,
} from "./testFixtures";

function harness(userId = USER_ID) {
	const store = new ProgrammerPriorityStore();
	const transport = new FakeProgrammerPriorityTransport();
	const onError = vi.fn();
	const session = new ProgrammerPrioritySession({
		scope: { userId },
		authorityKey: "session-a",
		store,
		transport,
		onError,
	});
	return { store, transport, onError, session };
}

describe("ProgrammerPrioritySession", () => {
	it("stays dormant, shares activation, and subscribes only to the user", async () => {
		const current = harness();
		await settlePrioritySession();
		expect(current.transport.loadSnapshot).not.toHaveBeenCalled();
		expect(current.transport.subscriptions).toHaveLength(0);

		const releaseFirst = current.session.activate();
		const releaseSecond = current.session.activate();
		await settlePrioritySession();
		expect(current.transport.loadSnapshot).toHaveBeenCalledOnce();
		expect(current.transport.loadSnapshot).toHaveBeenCalledWith({
			userId: USER_ID,
		});
		expect(current.transport.subscriptions).toHaveLength(1);
		expect(current.transport.subscriptions[0]).toMatchObject({
			scope: { userId: USER_ID },
			afterSequence: 10,
		});

		releaseFirst();
		await settlePrioritySession();
		expect(current.transport.subscriptions[0]?.close).not.toHaveBeenCalled();
		releaseSecond();
		await settlePrioritySession();
		expect(current.transport.subscriptions[0]?.close).toHaveBeenCalledOnce();
	});

	it("repairs a cursor gap with one narrow snapshot and stream repair", async () => {
		const current = harness();
		current.session.activate();
		await settlePrioritySession();
		current.transport.loadSnapshot.mockResolvedValueOnce(
			prioritySnapshot({ cursor: 20, revision: 3, priority: 12 }),
		);

		current.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settlePrioritySession();

		expect(current.transport.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);
		expect(current.store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			repairRequired: false,
			projection: { revision: 3, priority: 12 },
		});
	});

	it("rejects a foreign event and repairs exact-user authority", async () => {
		const current = harness();
		current.session.activate();
		await settlePrioritySession();
		current.transport.loadSnapshot.mockResolvedValueOnce(
			prioritySnapshot({ cursor: 20, revision: 1 }),
		);

		current.transport.emit({
			type: "event",
			sequence: 19,
			correlationId: null,
			change: {
				type: "upsert",
				projection: priorityProjection({
					userId: OTHER_USER_ID,
					revision: 9,
				}),
			},
		});
		await settlePrioritySession();

		expect(current.onError).toHaveBeenCalledWith(expect.any(Error));
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);
		expect(current.store.getSnapshot().projection?.userId).toBe(USER_ID);
	});

	it("applies removal tombstones and a later recreation", async () => {
		const current = harness();
		current.session.activate();
		await settlePrioritySession();

		current.transport.emit({
			type: "event",
			sequence: 11,
			correlationId: null,
			change: { type: "remove", userId: USER_ID, revision: 2 },
		});
		expect(current.store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			projection: null,
		});
		current.transport.emit({
			type: "event",
			sequence: 12,
			correlationId: null,
			change: {
				type: "upsert",
				projection: priorityProjection({ revision: 2, priority: 8 }),
			},
		});
		expect(current.store.getSnapshot()).toMatchObject({
			eventSequence: 12,
			projection: { revision: 2, priority: 8 },
		});
	});

	it("converges the same user on two desks while another user stays isolated", async () => {
		const transport = new FakeProgrammerPriorityTransport();
		const stores = [
			new ProgrammerPriorityStore(),
			new ProgrammerPriorityStore(),
			new ProgrammerPriorityStore(),
		];
		const users = [USER_ID, USER_ID, OTHER_USER_ID];
		const sessions = stores.map(
			(store, index) =>
				new ProgrammerPrioritySession({
					scope: { userId: users[index] ?? USER_ID },
					authorityKey: `desk-${index}`,
					store,
					transport,
				}),
		);
		for (const session of sessions) session.activate();
		await settlePrioritySession();
		expect(transport.subscriptions.map(({ scope }) => scope)).toEqual([
			{ userId: USER_ID },
			{ userId: USER_ID },
			{ userId: OTHER_USER_ID },
		]);
		const sameUserEvent = {
			type: "event" as const,
			sequence: 11,
			correlationId: null,
			change: {
				type: "upsert" as const,
				projection: priorityProjection({ revision: 2, priority: 55 }),
			},
		};
		transport.emit(sameUserEvent, 0);
		transport.emit(sameUserEvent, 1);

		expect(stores[0]?.getSnapshot().projection?.priority).toBe(55);
		expect(stores[1]?.getSnapshot().projection?.priority).toBe(55);
		expect(stores[2]?.getSnapshot()).toMatchObject({
			userId: OTHER_USER_ID,
			projection: { userId: OTHER_USER_ID, priority: 0 },
		});
	});

	it("cannot install a late snapshot after session replacement", async () => {
		const store = new ProgrammerPriorityStore();
		const transport = new FakeProgrammerPriorityTransport();
		const pending = deferred<ProgrammerPrioritySnapshot>();
		transport.loadSnapshot.mockReturnValueOnce(pending.promise);
		const session = new ProgrammerPrioritySession({
			scope: { userId: USER_ID },
			authorityKey: "session-a",
			store,
			transport,
		});
		session.activate();
		await Promise.resolve();

		session.stop();
		store.reset(OTHER_USER_ID, "session-b");
		pending.resolve(prioritySnapshot({ revision: 99 }));
		await settlePrioritySession();

		expect(store.getSnapshot()).toMatchObject({
			userId: OTHER_USER_ID,
			authorityKey: "session-b",
			projection: null,
		});
		expect(transport.subscriptions).toHaveLength(0);
	});
});
