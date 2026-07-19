import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackIdentity,
	PlaybackRuntimeEventMessage,
} from "./contracts";
import { playbackIdentity } from "./contracts";
import { PlaybackRuntimeSession } from "./session";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	DESK_ID,
	deskProjection,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";
import {
	type PlaybackEventObserver,
	type PlaybackEventScope,
	type PlaybackEventTransport,
	PlaybackProtocolError,
} from "./transport";

class FakeTransport implements PlaybackEventTransport {
	readonly subscriptions: Array<{
		scope: PlaybackEventScope;
		after: number | null;
		observer: PlaybackEventObserver;
		close: ReturnType<typeof vi.fn>;
		repair: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_deskId: string,
		scope: PlaybackEventScope,
		after: number | null,
		observer: PlaybackEventObserver,
	) {
		const subscription = {
			scope,
			after,
			observer,
			close: vi.fn(),
			repair: vi.fn(),
		};
		this.subscriptions.push(subscription);
		return subscription;
	}

	emit(message: PlaybackRuntimeEventMessage) {
		this.subscriptions.at(-1)?.observer.message(message);
	}
}

function createHarness(
	transport: PlaybackEventTransport | null = new FakeTransport(),
) {
	const store = new PlaybackRuntimeStore();
	const loadSnapshot = vi.fn(async (identities: PlaybackIdentity[]) =>
		playbackSnapshot(identities),
	);
	const onError = vi.fn();
	const session = new PlaybackRuntimeSession({
		showId: SHOW_ID,
		deskId: DESK_ID,
		store,
		transport,
		loadSnapshot,
		onError,
	});
	return { session, store, transport, loadSnapshot, onError };
}

async function settle() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

describe("PlaybackRuntimeSession", () => {
	it("batches mounted identities into one hydration and exact subscription", async () => {
		const harness = createHarness();
		const releaseOne = harness.session.activate(playbackIdentity(1));
		const releaseTwo = harness.session.activate(playbackIdentity(2));
		harness.session.activateDesk();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.loadSnapshot).toHaveBeenCalledWith([
			playbackIdentity(1),
			playbackIdentity(2),
		]);
		const transport = harness.transport as FakeTransport;
		expect(transport.subscriptions[0]).toMatchObject({
			after: 10,
			scope: {
				identities: [playbackIdentity(1), playbackIdentity(2)],
				desk: true,
			},
		});
		releaseOne();
		releaseTwo();
		await settle();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("hydrates over REST when WebSocket transport is unavailable", async () => {
		const harness = createHarness(null);
		harness.session.activate(playbackIdentity(3));
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledWith([playbackIdentity(3)]);
		expect(
			harness.store.getSnapshot().projections.get("playback:3")?.length,
		).toBe(1);
		expect(harness.store.getSnapshot().status).toBe("ready");
	});

	it("hydrates and repairs a desk-only view with no runtime identities", async () => {
		const harness = createHarness();
		harness.session.activateDesk();
		await settle();

		expect(harness.loadSnapshot).toHaveBeenCalledWith([]);
		const transport = harness.transport as FakeTransport;
		expect(transport.subscriptions[0]).toMatchObject({
			after: 10,
			scope: { identities: [], desk: true },
		});
		harness.loadSnapshot.mockResolvedValueOnce({
			...playbackSnapshot([], 18),
			desk: deskProjection(3),
		});
		transport.emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 14,
			latestSequence: 17,
		});
		await settle();

		expect(transport.subscriptions[0].repair).toHaveBeenCalledWith(18);
		expect(harness.store.getSnapshot().desk?.active_page).toBe(3);
	});

	it("does not install or notify for an irrelevant delivered identity", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1));
		await settle();
		const listener = vi.fn();
		harness.store.subscribe(listener);
		(harness.transport as FakeTransport).emit({
			type: "event",
			sequence: 11,
			payload: { type: "runtime", projection: cueProjection(2, 3) },
		});
		expect(listener).not.toHaveBeenCalled();
		expect(harness.store.getSnapshot().projections.has("playback:2")).toBe(
			false,
		);
	});

	it("repairs a gap from a fresh authoritative cursor before resuming", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1));
		await settle();
		harness.loadSnapshot.mockResolvedValueOnce(
			playbackSnapshot([playbackIdentity(1)], 18, [cueProjection(1, 4)]),
		);
		(harness.transport as FakeTransport).emit({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 14,
			latestSequence: 17,
		});
		await settle();
		const stream = (harness.transport as FakeTransport).subscriptions[0];
		expect(stream.repair).toHaveBeenCalledWith(18);
		const projection = harness.store
			.getSnapshot()
			.projections.get("playback:1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.cue_index,
		).toBe(4);
	});

	it("repairs a malformed event through REST without trusting its sequence", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1));
		await settle();
		harness.loadSnapshot.mockResolvedValueOnce(
			playbackSnapshot([playbackIdentity(1)], 20, [cueProjection(1, 5)]),
		);
		const first = (harness.transport as FakeTransport).subscriptions[0];
		first.observer.error(new PlaybackProtocolError("malformed", 19));
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect((harness.transport as FakeTransport).subscriptions[1].after).toBe(
			20,
		);
		expect(harness.store.getSnapshot().eventSequence).toBe(20);
	});

	it("splits large mounted views into bounded targeted snapshots", async () => {
		const harness = createHarness(null);
		for (let number = 1; number <= 300; number++)
			harness.session.activate(playbackIdentity(number));
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.loadSnapshot.mock.calls[0][0]).toHaveLength(256);
		expect(harness.loadSnapshot.mock.calls[1][0]).toHaveLength(44);
	});
});
