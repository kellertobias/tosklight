import { describe, expect, it, vi } from "vitest";
import type {
	PlaybackIdentity,
	PlaybackRuntimeEventMessage,
	PlaybackSnapshot,
} from "./contracts";
import { groupIdentity, playbackIdentity } from "./contracts";
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
		authorityKey: "authority-a",
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
		const releaseOne = harness.session.activate(playbackIdentity(1, 1));
		const releaseTwo = harness.session.activate(playbackIdentity(1, 2));
		harness.session.activateDesk();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(harness.loadSnapshot).toHaveBeenCalledWith([
			playbackIdentity(1, 1),
			playbackIdentity(1, 2),
		]);
		const transport = harness.transport as FakeTransport;
		expect(transport.subscriptions[0]).toMatchObject({
			after: 10,
			scope: {
				identities: [playbackIdentity(1, 1), playbackIdentity(1, 2)],
				desk: true,
			},
		});
		releaseOne();
		releaseTwo();
		await settle();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("installs an absent Group as resolved and does not re-request it", async () => {
		vi.useFakeTimers();
		try {
			const harness = createHarness();
			const stale = groupIdentity("from-previous-show");
			harness.loadSnapshot.mockImplementation(async (identities) =>
				playbackSnapshot(identities, 10, [
					{
						scope: { show_id: SHOW_ID, show_revision: 4 },
						identity: stale,
						target: "missing",
					},
				]),
			);
			harness.session.activate(stale);
			await settle();
			await vi.advanceTimersByTimeAsync(5_000);

			expect(harness.loadSnapshot).toHaveBeenCalledOnce();
			expect(harness.store.getSnapshot().status).toBe("ready");
			expect(harness.onError).not.toHaveBeenCalledWith(expect.any(Error));
			expect(
				harness.store.getSnapshot().projections.get("group:from-previous-show"),
			).toEqual([expect.objectContaining({ target: "missing" })]);
		} finally {
			vi.useRealTimers();
		}
	});

	it("keeps retrying a failed snapshot as the negative control", async () => {
		vi.useFakeTimers();
		try {
			const harness = createHarness();
			harness.loadSnapshot.mockRejectedValue(new Error("Group not found"));
			harness.session.activate(groupIdentity("from-previous-show"));
			await settle();
			await vi.advanceTimersByTimeAsync(2_000);

			expect(harness.loadSnapshot.mock.calls.length).toBeGreaterThan(1);
			expect(harness.store.getSnapshot().status).toBe("error");
		} finally {
			vi.useRealTimers();
		}
	});

	it("hydrates over REST when WebSocket transport is unavailable", async () => {
		const harness = createHarness(null);
		harness.session.activate(playbackIdentity(1, 3));
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledWith([playbackIdentity(1, 3)]);
		expect(
			harness.store.getSnapshot().projections.get("playback:1.3")?.length,
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
		expect(harness.store.getSnapshot().desk?.command_page).toBe(3);
	});

	it("refreshes an already-mounted desk authority before guarded mutations", async () => {
		const harness = createHarness();
		harness.session.activateDesk();
		await settle();
		harness.loadSnapshot.mockResolvedValueOnce({
			...playbackSnapshot([], 19),
			desk: {
				...deskProjection(2),
				scope: { show_id: SHOW_ID, show_revision: 7 },
			},
		});

		await harness.session.refreshAuthority();

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.store.getSnapshot().showRevision).toBe(7);
	});

	it("keeps one desk subscription while duplicate consumers mount and release", async () => {
		const harness = createHarness();
		const releaseFirst = harness.session.activateDesk();
		await settle();
		const transport = harness.transport as FakeTransport;

		const releaseSecond = harness.session.activateDesk();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);

		releaseSecond();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].close).not.toHaveBeenCalled();

		releaseFirst();
		await settle();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("keeps one runtime subscription while duplicate identity consumers mount and release", async () => {
		const harness = createHarness();
		const identity = playbackIdentity(1, 1);
		const releaseFirst = harness.session.activate(identity);
		await settle();
		const transport = harness.transport as FakeTransport;

		const releaseSecond = harness.session.activate(identity);
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);

		releaseFirst();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].close).not.toHaveBeenCalled();

		releaseSecond();
		await settle();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("retains hydrated runtime identities across a dormant view lifetime", async () => {
		const harness = createHarness();
		const identity = playbackIdentity(1, 1);
		const releaseWarmLease = harness.session.activate(identity);
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();

		const releaseView = harness.session.activate(identity);
		await settle();
		releaseView();
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(
			(harness.transport as FakeTransport).subscriptions[0].close,
		).not.toHaveBeenCalled();

		const releaseRemountedView = harness.session.activate(identity);
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		releaseRemountedView();
		releaseWarmLease();
		await settle();
		expect(
			(harness.transport as FakeTransport).subscriptions[0].close,
		).toHaveBeenCalledOnce();
	});

	it("upgrades a warm lease to visible telemetry without reloading its snapshot", async () => {
		const harness = createHarness();
		const transport = harness.transport as FakeTransport;
		const identity = playbackIdentity(1, 1);
		const releaseWarmLease = harness.session.activate(identity, false);
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions[0].scope).toEqual({
			identities: [identity],
			desk: false,
			telemetry: false,
		});

		const releaseVisibleView = harness.session.activate(identity);
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1].scope).toEqual({
			identities: [identity],
			desk: false,
		});

		releaseVisibleView();
		await settle();
		expect(transport.subscriptions[2].scope).toEqual({
			identities: [identity],
			desk: false,
			telemetry: false,
		});
		releaseWarmLease();
	});

	it("hydrates only a newly added identity and resumes events from the retained cursor", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1, 1));
		await settle();
		harness.session.activate(playbackIdentity(1, 2));
		await settle();

		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.loadSnapshot).toHaveBeenNthCalledWith(1, [
			playbackIdentity(1, 1),
		]);
		expect(harness.loadSnapshot).toHaveBeenNthCalledWith(2, [
			playbackIdentity(1, 2),
		]);
		expect((harness.transport as FakeTransport).subscriptions[1]).toMatchObject(
			{
				after: 10,
				scope: {
					identities: [playbackIdentity(1, 1), playbackIdentity(1, 2)],
				},
			},
		);
	});

	it("does not install or notify for an irrelevant delivered identity", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1, 1));
		await settle();
		const listener = vi.fn();
		harness.store.subscribe(listener);
		(harness.transport as FakeTransport).emit({
			type: "event",
			sequence: 11,
			payload: { type: "runtime", projection: cueProjection(2, 3) },
		});
		expect(listener).not.toHaveBeenCalled();
		expect(harness.store.getSnapshot().projections.has("playback:1.2")).toBe(
			false,
		);
	});

	it.each([
		[
			"foreign-show",
			{
				...cueProjection(1),
				scope: {
					...cueProjection(1).scope,
					show_id: "99999999-9999-4999-8999-999999999999",
				},
			},
		],
		["unrequested", cueProjection(2)],
	])("rejects a %s snapshot projection", async (_label, projection) => {
		const harness = createHarness();
		const identity = playbackIdentity(1, 1);
		harness.loadSnapshot.mockResolvedValueOnce(
			playbackSnapshot([identity], 10, [projection]),
		);

		harness.session.activate(identity);
		await settle();

		expect(harness.store.getSnapshot().projections.size).toBe(0);
		expect(harness.store.getSnapshot().status).toBe("error");
		expect(harness.onError).toHaveBeenCalledWith(
			expect.any(PlaybackProtocolError),
		);
		harness.session.stop();
	});

	it("repairs a gap from a fresh authoritative cursor before resuming", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1, 1));
		await settle();
		harness.loadSnapshot.mockResolvedValueOnce(
			playbackSnapshot([playbackIdentity(1, 1)], 18, [cueProjection(1, 4)]),
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
			.projections.get("playback:1.1")?.[0];
		expect(
			projection?.target === "cue_list" && projection.runtime?.cue_index,
		).toBe(4);
	});

	it("repairs a malformed event through REST without trusting its sequence", async () => {
		const harness = createHarness();
		harness.session.activate(playbackIdentity(1, 1));
		await settle();
		harness.loadSnapshot.mockResolvedValueOnce(
			playbackSnapshot([playbackIdentity(1, 1)], 20, [cueProjection(1, 5)]),
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
			harness.session.activate(playbackIdentity(1, number));
		await settle();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(harness.loadSnapshot.mock.calls[0][0]).toHaveLength(256);
		expect(harness.loadSnapshot.mock.calls[1][0]).toHaveLength(44);
	});
	it("does not count a repair superseded by a subscription change as fresh authority", async () => {
		const h = createHarness(null);
		const identity = playbackIdentity(1, 1);
		h.session.activate(identity);
		await settle();
		let resolve!: (value: PlaybackSnapshot) => void;
		h.loadSnapshot.mockImplementationOnce(
			() =>
				new Promise((done) => {
					resolve = done;
				}),
		);
		const refresh = h.session.refreshAuthority();
		// Desk state is already hydrated, so this changes lifecycle without refreshing identity1.
		h.session.activateDesk();
		await settle();
		const next = playbackSnapshot([identity], 20);
		const target = "55555555-5555-4555-8555-555555555555";
		next.projections = next.projections.map((projection) =>
			projection.target === "cue_list"
				? { ...projection, cue_list_id: target }
				: projection,
		);
		h.loadSnapshot.mockResolvedValueOnce(next);
		resolve(next);
		await refresh;
		expect(h.loadSnapshot).toHaveBeenCalledTimes(3);
		expect(
			h.store.getSnapshot().projections.get("playback:1.1")?.[0],
		).toMatchObject({ target: "cue_list", cue_list_id: target });
		h.session.stop();
	});
	it("coalesces callers across repeated subscription changes without busy retries", async () => {
		const h = createHarness(null);
		const first = playbackIdentity(1, 1);
		h.session.activate(first);
		await settle();
		const pending: Array<{
			identities: PlaybackIdentity[];
			resolve(value: PlaybackSnapshot): void;
		}> = [];
		h.loadSnapshot.mockImplementation(
			(identities) =>
				new Promise((resolve) => pending.push({ identities, resolve })),
		);
		let completed = 0;
		const one = h.session.refreshAuthority().then(() => {
			completed++;
		});
		const two = h.session.refreshAuthority().then(() => {
			completed++;
		});
		expect(pending).toHaveLength(1);
		h.session.activateDesk();
		await settle();
		pending[0].resolve(playbackSnapshot(pending[0].identities, 20));
		await vi.waitFor(() => expect(pending).toHaveLength(2));
		expect(completed).toBe(0);
		const second = playbackIdentity(1, 2);
		h.session.activate(second);
		await settle();
		expect(pending).toHaveLength(3);
		pending[2].resolve(playbackSnapshot(pending[2].identities, 21));
		await settle();
		pending[1].resolve(playbackSnapshot(pending[1].identities, 22));
		await vi.waitFor(() => expect(pending).toHaveLength(4));
		await settle();
		expect(pending).toHaveLength(4);
		expect(completed).toBe(0);
		expect(pending[3].identities).toEqual([first, second]);
		pending[3].resolve(playbackSnapshot(pending[3].identities, 23));
		await Promise.all([one, two]);
		expect(completed).toBe(2);
		expect(h.loadSnapshot).toHaveBeenCalledTimes(5);
		h.session.stop();
	});
	it.each([
		"resolve",
		"reject",
	] as const)("retires a %s from an old show without changing the new authority", async (outcome) => {
		const h = createHarness(null);
		h.session.activate(playbackIdentity(1, 1));
		await settle();
		let resolve!: (snapshot: PlaybackSnapshot) => void;
		let reject!: (error: Error) => void;
		h.loadSnapshot.mockImplementationOnce(
			() =>
				new Promise((yes, no) => {
					resolve = yes;
					reject = no;
				}),
		);
		const refresh = h.session.refreshAuthority();
		const rejected = expect(refresh).rejects.toThrow(
			"authority is unavailable",
		);
		h.store.reset(
			"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
			DESK_ID,
			"authority-b",
		);
		h.store.setReady();
		const next = h.store.getSnapshot();
		h.onError.mockClear();
		if (outcome === "resolve")
			resolve(playbackSnapshot([playbackIdentity(1, 1)], 20));
		else reject(new Error("Retired failure"));
		await rejected;
		expect(h.store.getSnapshot()).toBe(next);
		expect(h.onError).not.toHaveBeenCalled();
		expect(h.loadSnapshot).toHaveBeenCalledTimes(2);
		h.session.stop();
	});
	it("stops an in-flight freshness wait when the session is disposed", async () => {
		const h = createHarness(null);
		h.session.activateDesk();
		await settle();
		let resolve!: (snapshot: PlaybackSnapshot) => void;
		h.loadSnapshot.mockImplementationOnce(
			() =>
				new Promise((done) => {
					resolve = done;
				}),
		);
		const refresh = h.session.refreshAuthority();
		const rejected = expect(refresh).rejects.toThrow(
			"authority is unavailable",
		);
		h.session.stop();
		const previous = h.store.getSnapshot();
		resolve(playbackSnapshot([], 20));
		await rejected;
		expect(h.store.getSnapshot()).toBe(previous);
		expect(h.loadSnapshot).toHaveBeenCalledTimes(2);
	});
	it("propagates a current snapshot error even if automatic recovery starts", async () => {
		const h = createHarness(null);
		h.session.activateDesk();
		await settle();
		const failure = new Error("Actual refresh failure");
		h.loadSnapshot.mockRejectedValueOnce(failure);
		await expect(h.session.refreshAuthority()).rejects.toBe(failure);
		expect(h.onError).toHaveBeenCalledWith(failure);
		h.session.stop();
	});
	it("retries a superseded request failure without reporting it against the current generation", async () => {
		const h = createHarness(null);
		h.session.activate(playbackIdentity(1, 1));
		await settle();
		let reject!: (error: Error) => void;
		h.loadSnapshot.mockImplementationOnce(
			() =>
				new Promise((_yes, no) => {
					reject = no;
				}),
		);
		const refresh = h.session.refreshAuthority();
		h.session.activateDesk();
		await settle();
		h.onError.mockClear();
		reject(new Error("Superseded request failure"));
		await refresh;
		expect(h.loadSnapshot).toHaveBeenCalledTimes(3);
		expect(h.onError).not.toHaveBeenCalledWith(expect.any(Error));
		h.session.stop();
	});
	it("retains a concurrent current stream error after the fresh snapshot installs", async () => {
		const h = createHarness();
		h.session.activateDesk();
		await settle();
		const fresh = playbackSnapshot([], 20);
		fresh.desk.scope.show_revision = 7;
		h.loadSnapshot.mockResolvedValueOnce(fresh);
		const failure = new Error("Current stream failure");
		let fired = false;
		const release = h.store.subscribe(() => {
			if (!fired && h.store.getSnapshot().showRevision === 7) {
				fired = true;
				(h.transport as FakeTransport).subscriptions
					.at(-1)
					?.observer.error(failure);
			}
		});
		await expect(h.session.refreshAuthority()).rejects.toBe(failure);
		expect(fired).toBe(true);
		release();
		h.session.stop();
	});
});
