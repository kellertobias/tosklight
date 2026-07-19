import { describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionSession } from "./session";
import { ProgrammingInteractionStore } from "./store";
import {
	commandChange,
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	programmingSnapshot,
	selectionChange,
	settleSession,
	SHOW_ID,
} from "./testFixtures";
import {
	type ProgrammingEventTransport,
	ProgrammingProtocolError,
} from "./transport";

function createHarness(
	transport: ProgrammingEventTransport | null = new FakeProgrammingTransport(),
) {
	const store = new ProgrammingInteractionStore();
	const loadSnapshot = vi.fn(async () => programmingSnapshot());
	const onError = vi.fn();
	const session = new ProgrammingInteractionSession({
		showId: SHOW_ID,
		deskId: DESK_ID,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { session, store, transport, loadSnapshot, onError };
}

describe("ProgrammingInteractionSession scope", () => {
	it("does not hydrate or open a stream with no mounted capability", async () => {
		const harness = createHarness();

		await settleSession();

		expect(harness.loadSnapshot).not.toHaveBeenCalled();
		expect(
			(harness.transport as FakeProgrammingTransport).subscriptions,
		).toHaveLength(0);
	});

	it("reference-counts capabilities without reopening unchanged scopes", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammingTransport;
		const releaseFirstCommand = harness.session.activate("commandLine");
		await settleSession();

		expect(transport.subscriptions[0]).toMatchObject({
			deskId: DESK_ID,
			after: 10,
			scope: { commandLine: true, selection: false },
		});

		const releaseSecondCommand = harness.session.activate("commandLine");
		await settleSession();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);

		const releaseSelection = harness.session.activate("selection");
		await settleSession();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1]).toMatchObject({
			after: 10,
			scope: { commandLine: true, selection: true },
		});

		releaseFirstCommand();
		await settleSession();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions).toHaveLength(2);

		releaseSecondCommand();
		await settleSession();
		expect(transport.subscriptions[1].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[2]).toMatchObject({
			after: 10,
			scope: { commandLine: false, selection: true },
		});

		releaseSelection();
		await settleSession();
		expect(transport.subscriptions[2].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(3);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(3);
	});

	it("hydrates over REST when the WebSocket transport is unavailable", async () => {
		const harness = createHarness(null);
		harness.session.activate("selection");

		await settleSession();

		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.store.getSnapshot()).toMatchObject({
			status: "ready",
			eventSequence: 10,
			selection: { revision: 1 },
		});
	});
});

describe("ProgrammingInteractionSession events", () => {
	it("accepts filtered global sequence gaps and ignores unrelated changes", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammingTransport;
		harness.session.activate("commandLine");
		await settleSession();
		const listener = vi.fn();
		harness.store.subscribe(listener);

		transport.emit({
			type: "event",
			sequence: 31,
			change: selectionChange({ revision: 3 }),
		});
		expect(listener).not.toHaveBeenCalled();
		expect(harness.store.getSnapshot().eventSequence).toBe(10);

		transport.emit({
			type: "event",
			sequence: 47,
			change: commandChange({ revision: 2, text: "FIXTURE 47" }),
		});
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 47,
			commandLine: { revision: 2, text: "FIXTURE 47" },
		});
	});

	it("installs an authoritative gap snapshot before repairing the stream", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammingTransport;
		harness.session.activate("commandLine");
		await settleSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			programmingSnapshot({
				sequence: 18,
				command: commandLine(4, "FIXTURE 18"),
			}),
		);
		const stream = transport.subscriptions[0];
		let revisionAtRepair: number | null = null;
		stream.repair.mockImplementation(() => {
			revisionAtRepair =
				harness.store.getSnapshot().commandLine?.revision ?? null;
		});

		transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 14,
			latestSequence: 17,
		});
		await settleSession();

		expect(revisionAtRepair).toBe(4);
		expect(stream.repair).toHaveBeenCalledWith(18);
		expect(harness.store.getSnapshot()).toMatchObject({
			eventSequence: 18,
			commandLine: { revision: 4, text: "FIXTURE 18" },
		});
	});

	it("rehydrates from REST and opens a new stream after a protocol error", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeProgrammingTransport;
		harness.session.activate("commandLine");
		await settleSession();
		harness.loadSnapshot.mockResolvedValueOnce(
			programmingSnapshot({
				sequence: 20,
				command: commandLine(5, "GROUP 20"),
			}),
		);
		const first = transport.subscriptions[0];
		const malformed = new ProgrammingProtocolError("malformed event", 19);

		first.observer.error(malformed);
		await settleSession();

		expect(first.close).toHaveBeenCalledOnce();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions).toHaveLength(2);
		expect(transport.subscriptions[1]).toMatchObject({
			after: 20,
			scope: { commandLine: true, selection: false },
		});
		expect(harness.store.getSnapshot()).toMatchObject({
			status: "ready",
			error: null,
			eventSequence: 20,
			commandLine: { revision: 5, text: "GROUP 20" },
		});
		expect(harness.onError).toHaveBeenCalledWith(malformed);
		expect(harness.onError).toHaveBeenLastCalledWith(null);
	});
});
