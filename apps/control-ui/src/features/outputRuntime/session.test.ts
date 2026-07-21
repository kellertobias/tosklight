import { describe, expect, it, vi } from "vitest";
import type { OutputRuntimeSnapshot } from "./contracts";
import { OutputRuntimeSession } from "./session";
import { OutputRuntimeStore } from "./store";
import {
	deferred,
	DESK_ID,
	FakeOutputRuntimeTransport,
	OTHER_DESK_ID,
	OTHER_SHOW_ID,
	outputProjection,
	outputSnapshot,
	settleOutputSession,
	SHOW_ID,
} from "./testFixtures";
import { OutputRuntimeProtocolError } from "./transport";

function harness(showId = SHOW_ID, deskId = DESK_ID) {
	const store = new OutputRuntimeStore();
	const transport = new FakeOutputRuntimeTransport();
	const onError = vi.fn();
	const session = new OutputRuntimeSession({
		scope: { showId, deskId },
		authorityKey: "session-a",
		store,
		transport,
		onError,
	});
	return { store, transport, onError, session };
}

describe("OutputRuntimeSession", () => {
	it("stays dormant, shares activation, and scopes snapshot and stream", async () => {
		const current = harness();
		await settleOutputSession();
		expect(current.transport.loadSnapshot).not.toHaveBeenCalled();
		expect(current.transport.subscriptions).toHaveLength(0);

		const releaseFirst = current.session.activate();
		const releaseSecond = current.session.activate();
		await settleOutputSession();
		expect(current.transport.loadSnapshot).toHaveBeenCalledOnce();
		expect(current.transport.loadSnapshot).toHaveBeenCalledWith({
			showId: SHOW_ID,
			deskId: DESK_ID,
		});
		expect(current.transport.subscriptions).toHaveLength(1);
		expect(current.transport.subscriptions[0]).toMatchObject({
			scope: { showId: SHOW_ID, deskId: DESK_ID },
			afterSequence: 10,
		});

		releaseFirst();
		await settleOutputSession();
		expect(current.transport.subscriptions[0]?.close).not.toHaveBeenCalled();
		releaseSecond();
		await settleOutputSession();
		expect(current.transport.subscriptions[0]?.close).toHaveBeenCalledOnce();
	});

	it("repairs a cursor gap and malformed socket message narrowly", async () => {
		const current = harness();
		current.session.activate();
		await settleOutputSession();
		current.transport.loadSnapshot.mockResolvedValueOnce(
			outputSnapshot({
				cursor: 20,
				revision: 3,
				grandMaster: 0.6,
			}),
		);

		current.transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 15,
			latestSequence: 19,
		});
		await settleOutputSession();
		expect(current.transport.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);

		current.transport.loadSnapshot.mockResolvedValueOnce(
			outputSnapshot({
				cursor: 21,
				revision: 3,
				grandMaster: 0.6,
			}),
		);
		current.transport.subscriptions[0]?.observer.error(
			new OutputRuntimeProtocolError("malformed event", 21),
		);
		await settleOutputSession();
		expect(current.transport.loadSnapshot).toHaveBeenCalledTimes(3);
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenLastCalledWith(
			21,
		);
	});

	it("rejects a foreign-Show event and repairs active authority", async () => {
		const current = harness();
		current.session.activate();
		await settleOutputSession();
		current.transport.loadSnapshot.mockResolvedValueOnce(
			outputSnapshot({ cursor: 20, revision: 1 }),
		);

		current.transport.emit({
			type: "event",
			sequence: 19,
			correlationId: null,
			change: {
				projection: outputProjection({
					showId: OTHER_SHOW_ID,
					revision: 9,
				}),
			},
		});
		await settleOutputSession();

		expect(current.onError).toHaveBeenCalledWith(expect.any(Error));
		expect(current.transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);
		expect(current.store.getSnapshot().projection?.showId).toBe(SHOW_ID);
	});

	it("converges one installation value across desks while Show scopes isolate", async () => {
		const transport = new FakeOutputRuntimeTransport();
		const stores = [
			new OutputRuntimeStore(),
			new OutputRuntimeStore(),
			new OutputRuntimeStore(),
		];
		const scopes = [
			{ showId: SHOW_ID, deskId: DESK_ID },
			{ showId: SHOW_ID, deskId: OTHER_DESK_ID },
			{ showId: OTHER_SHOW_ID, deskId: DESK_ID },
		];
		const sessions = stores.map(
			(store, index) =>
				new OutputRuntimeSession({
					scope: scopes[index] ?? scopes[0]!,
					authorityKey: `desk-${index}`,
					store,
					transport,
				}),
		);
		for (const session of sessions) session.activate();
		await settleOutputSession();
		const sameInstallationEvent = {
			type: "event" as const,
			sequence: 11,
			correlationId: null,
			change: {
				projection: outputProjection({
					revision: 2,
					grandMaster: 0.55,
				}),
			},
		};
		transport.emit(sameInstallationEvent, 0);
		transport.emit(sameInstallationEvent, 1);

		expect(stores[0]?.getSnapshot().projection?.grandMaster).toBe(0.55);
		expect(stores[1]?.getSnapshot().projection?.grandMaster).toBe(0.55);
		expect(stores[2]?.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			projection: { showId: OTHER_SHOW_ID, grandMaster: 1 },
		});
	});

	it("cannot install a late snapshot after session replacement", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const pending = deferred<OutputRuntimeSnapshot>();
		transport.loadSnapshot.mockReturnValueOnce(pending.promise);
		const session = new OutputRuntimeSession({
			scope: { showId: SHOW_ID, deskId: DESK_ID },
			authorityKey: "session-a",
			store,
			transport,
		});
		session.activate();
		await Promise.resolve();

		session.stop();
		store.reset(OTHER_SHOW_ID, OTHER_DESK_ID, "session-b");
		pending.resolve(outputSnapshot({ revision: 99 }));
		await settleOutputSession();

		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			deskId: OTHER_DESK_ID,
			authorityKey: "session-b",
			projection: null,
		});
		expect(transport.subscriptions).toHaveLength(0);
	});
});
