import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";
import { VisualizationRuntimeSession } from "./session";
import { VisualizationRuntimeStore } from "./store";
import type { VisualizationRuntimeTransport } from "./transport";

const scope: VisualizationRuntimeScope = {
	showId: "11111111-1111-4111-8111-111111111111",
	sessionId: "22222222-2222-4222-8222-222222222222",
	authorityKey: "server-a",
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("VisualizationRuntimeSession", () => {
	it("is dormant until claimed and shares one fastest normal poll", async () => {
		const harness = createHarness();
		await flush();
		expect(harness.loadSnapshot).not.toHaveBeenCalled();

		const releaseSlow = harness.session.activate("normal", 400);
		const releaseFast = harness.session.activate("normal", 200);
		await flush();
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(1);
		expect(harness.loadSnapshot.mock.calls[0]?.[1]).toBe("normal");

		await vi.advanceTimersByTimeAsync(199);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(1);
		await vi.advanceTimersByTimeAsync(1);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);

		releaseFast();
		await vi.advanceTimersByTimeAsync(399);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
		await vi.advanceTimersByTimeAsync(1);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(3);
		releaseSlow();
		await vi.advanceTimersByTimeAsync(1_000);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(3);
	});

	it("never overlaps a slow request in one lane", async () => {
		const pending = deferred<VisualizationSnapshot>();
		const harness = createHarness((lane) =>
			lane === "normal" ? pending.promise : Promise.resolve(snapshot(lane)),
		);
		harness.session.activate("normal", 200);
		await flush();

		await vi.advanceTimersByTimeAsync(1_000);
		expect(harness.loadSnapshot).toHaveBeenCalledOnce();
		pending.resolve(snapshot("normal"));
		await pending.promise;
		await vi.advanceTimersByTimeAsync(200);
		expect(harness.loadSnapshot).toHaveBeenCalledTimes(2);
	});

	it("polls normal and preload as independent claimed lanes", async () => {
		const harness = createHarness();
		harness.session.activate("normal", 250);
		harness.session.activate("preload", 250);
		await flush();

		expect(
			harness.loadSnapshot.mock.calls.map((call) => call[1]).sort(),
		).toEqual(["normal", "preload"]);
		expect(harness.store.getSnapshot().normal.snapshot?.preload).toBe(false);
		expect(harness.store.getSnapshot().preload.snapshot?.preload).toBe(true);
	});

	it("uses one multiplexed stream without periodic HTTP when available", async () => {
		const store = new VisualizationRuntimeStore();
		store.reset(scope);
		const updateClaims = vi.fn();
		const close = vi.fn();
		let observer:
			| Parameters<NonNullable<VisualizationRuntimeTransport["openStream"]>>[1]
			| undefined;
		const transport: VisualizationRuntimeTransport = {
			loadSnapshot: vi.fn(async (_scope, lane) => snapshot(lane)),
			openStream: vi.fn((_scope, nextObserver) => {
				observer = nextObserver;
				return { updateClaims, close };
			}),
		};
		const session = new VisualizationRuntimeSession({
			scope,
			store,
			transport,
		});

		const releaseNormal = session.activate("normal", 100);
		const releasePreload = session.activate("preload", 100);
		await vi.advanceTimersByTimeAsync(1_000);

		expect(transport.openStream).toHaveBeenCalledOnce();
		expect(transport.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(updateClaims).toHaveBeenLastCalledWith(
			["normal", "preload"],
			10,
			false,
		);
		observer?.snapshot("normal", snapshot("normal"));
		expect(store.getSnapshot().normal.status).toBe("ready");

		releasePreload();
		expect(updateClaims).toHaveBeenLastCalledWith(["normal"], 10, false);
		releaseNormal();
		expect(updateClaims).toHaveBeenLastCalledWith([], 10, false);
		expect(updateClaims.mock.invocationCallOrder.at(-1)).toBeLessThan(
			close.mock.invocationCallOrder[0],
		);
		expect(close).toHaveBeenCalledOnce();
	});

	it("unsubscribes the normal lane after its last duplicate claim is released", () => {
		const store = new VisualizationRuntimeStore();
		store.reset(scope);
		const updateClaims = vi.fn();
		const transport: VisualizationRuntimeTransport = {
			loadSnapshot: vi.fn(),
			openStream: vi.fn(() => ({ updateClaims, close: vi.fn() })),
		};
		const session = new VisualizationRuntimeSession({
			scope,
			store,
			transport,
		});
		const releaseFirstNormal = session.activate("normal", 100);
		const releaseSecondNormal = session.activate("normal", 100);
		session.activate("preload", 100);

		releaseFirstNormal();
		expect(updateClaims).toHaveBeenLastCalledWith(
			["normal", "preload"],
			10,
			false,
		);
		releaseSecondNormal();
		expect(updateClaims).toHaveBeenLastCalledWith(["preload"], 10, false);
	});

	it("claims the diagnostic Dynamic stack only while a requesting consumer is active", () => {
		const store = new VisualizationRuntimeStore();
		store.reset(scope);
		const updateClaims = vi.fn();
		const transport: VisualizationRuntimeTransport = {
			loadSnapshot: vi.fn(),
			openStream: vi.fn(() => ({ updateClaims, close: vi.fn() })),
		};
		const session = new VisualizationRuntimeSession({
			scope,
			store,
			transport,
		});
		session.activate("normal", 100, "stage");
		const releaseFixtureSheet = session.activate(
			"normal",
			250,
			"fixture-sheet",
			true,
		);

		expect(updateClaims).toHaveBeenLastCalledWith(["normal"], 10, true);
		releaseFixtureSheet();
		expect(updateClaims).toHaveBeenLastCalledWith(["normal"], 10, false);
	});

	it("drops an old response after immediate scope replacement", async () => {
		const pending = deferred<VisualizationSnapshot>();
		const harness = createHarness(() => pending.promise);
		harness.session.activate("normal", 250);
		await flush();

		harness.store.reset({ ...scope, authorityKey: "server-b" });
		expect(harness.store.getSnapshot().normal.snapshot).toBeNull();
		pending.resolve(snapshot("normal"));
		await pending.promise;

		expect(harness.store.getSnapshot()).toMatchObject({
			scope: { authorityKey: "server-b" },
			normal: { status: "idle", snapshot: null },
		});
	});

	it("cannot idle the replacement authority during old-claim cleanup", async () => {
		const harness = createHarness();
		const release = harness.session.activate("normal", 250);
		await flush();
		harness.store.reset({ ...scope, authorityKey: "server-b" });
		harness.store.setLoading("normal");

		release();

		expect(harness.store.getSnapshot().normal.status).toBe("loading");
	});
});

function createHarness(
	implementation: (
		lane: VisualizationRuntimeLane,
	) => Promise<VisualizationSnapshot> = async (lane) => snapshot(lane),
) {
	const store = new VisualizationRuntimeStore();
	store.reset(scope);
	const loadSnapshot = vi.fn(
		(_scope: VisualizationRuntimeScope, lane: VisualizationRuntimeLane) =>
			implementation(lane),
	);
	const transport: VisualizationRuntimeTransport = { loadSnapshot };
	return {
		store,
		loadSnapshot,
		session: new VisualizationRuntimeSession({ scope, store, transport }),
	};
}

function snapshot(lane: VisualizationRuntimeLane): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 1,
		blackout: false,
		preload: lane === "preload",
		values: [],
		profile_output_values: [],
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

async function flush() {
	await Promise.resolve();
	await Promise.resolve();
}
