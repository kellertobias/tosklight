import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
} from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesProjection,
} from "./contracts";
import { ProgrammerValuesStore } from "./store";
import {
	FIXTURE_1,
	fixtureValue,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

function harness(captureReady = true) {
	const store = new ProgrammerValuesStore();
	store.reset(SHOW_ID, USER_ID, "session-a");
	store.installSnapshot(valuesSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, USER_ID, "session-a");
	if (captureReady) captureModeStore.installSnapshot(captureModeSnapshot());
	const applyAction =
		vi.fn<
			(
				scope: { showId: string; userId: string },
				request: ProgrammerValuesActionRequest,
			) => Promise<ProgrammerValuesActionOutcome>
		>();
	const repair = vi.fn<(error: Error) => Promise<void>>(async () => undefined);
	const repairCaptureMode = vi.fn<(error: Error) => Promise<void>>(
		async () => undefined,
	);
	const onError = vi.fn();
	const writer = new ProgrammerValuesWriter({
		scope: { showId: SHOW_ID, userId: USER_ID },
		store,
		captureModeStore,
		applyAction,
		repair,
		repairCaptureMode,
		onError,
	});
	return {
		store,
		captureModeStore,
		applyAction,
		repair,
		repairCaptureMode,
		onError,
		writer,
	};
}

function changed(
	requestId: string,
	projection: ProgrammerValuesProjection,
	eventSequence = 20,
): ProgrammerValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "changed",
		revision: projection.revision,
		captureModeRevision: 1,
		projection,
		eventSequence,
		replayed: false,
		warning: null,
	};
}

function noChange(
	requestId: string,
	revision = 1,
): ProgrammerValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change",
		revision,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function fixtureInput(requestId: string, level: number) {
	return {
		requestId,
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: { kind: "normalized" as const, value: level },
		fade: false,
		fadeMillis: null,
		delayMillis: null,
	};
}

function fixtureLevel(store: ProgrammerValuesStore) {
	const value = store.getSnapshot().projection?.fixtureValues[0]?.value;
	return value?.kind === "normalized" ? value.value : null;
}

describe("ProgrammerValuesWriter reconciliation", () => {
	it("settles an HTTP response before the matching event", async () => {
		const { store, applyAction, writer } = harness();
		const projection = valuesProjection({
			revision: 2,
			fixtureValues: [fixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockResolvedValueOnce(changed("request-a", projection));

		await expect(
			writer.setFixtureValue(fixtureInput("request-a", 0.8)),
		).resolves.toMatchObject({ status: "changed" });
		expect(applyAction.mock.calls[0]?.[1]).toMatchObject({
			expectedRevision: 1,
			expectedCaptureModeRevision: 1,
		});

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		const beforeEvent = store.getSnapshot().projection;
		store.applyProjection(projection, 20);
		expect(store.getSnapshot().projection).toBe(beforeEvent);
	});

	it("keeps intent rebased when the event arrives before HTTP", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerValuesActionOutcome>();
		const projection = valuesProjection({
			revision: 2,
			fixtureValues: [fixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("request-a", 0.8));
		await Promise.resolve();

		store.applyProjection(projection, 20);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["request-a"]);
		expect(fixtureLevel(store)).toBe(0.8);
		response.resolve(changed("request-a", projection));
		await pending;

		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(store.getSnapshot().eventSequence).toBe(20);
	});

	it("tracks and sends a predicted no-op without cloning projection", async () => {
		const { store, applyAction, writer } = harness();
		const projection = store.getSnapshot().projection;
		applyAction.mockResolvedValueOnce(noChange("same-value"));

		const pending = writer.setFixtureValue(fixtureInput("same-value", 0.25));
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["same-value"]);
		await pending;

		expect(applyAction).toHaveBeenCalledOnce();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("replays an ambiguous request once with the identical body", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(new Error("connection reset"))
			.mockResolvedValueOnce(noChange("replay-a"));

		await writer.releaseFixtureValue({
			requestId: "replay-a",
			fixtureId: "22222222-2222-4222-8222-222222222222",
			attribute: "intensity",
		});

		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
	});

	it("sends a batch as one action and one network request", async () => {
		const { applyAction, writer } = harness();
		applyAction.mockResolvedValueOnce(noChange("batch-a"));

		await writer.batch({
			requestId: "batch-a",
			mutations: [
				{
					action: "set_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					timing: { fade: false, fadeMillis: null, delayMillis: null },
				},
				{
					action: "release_group",
					groupId: "front",
					attribute: "intensity",
				},
			],
		});

		expect(applyAction).toHaveBeenCalledOnce();
		expect(applyAction.mock.calls[0]?.[1].action).toMatchObject({
			action: "batch",
			mutations: [{ action: "set_fixture" }, { action: "release_group" }],
		});
	});

	it("uses the settled revision for the next queued write", async () => {
		const { applyAction, writer } = harness();
		const first = deferred<ProgrammerValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(first.promise)
			.mockResolvedValueOnce(noChange("second", 2));
		const firstWrite = writer.setFixtureValue(fixtureInput("first", 0.8));
		const secondWrite = writer.releaseFixtureValue({
			requestId: "second",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});
		await Promise.resolve();
		first.resolve(
			changed(
				"first",
				valuesProjection({ revision: 2, fixtureValues: [fixtureValue(0.8)] }),
			),
		);
		await Promise.all([firstWrite, secondWrite]);

		expect(applyAction.mock.calls[0]?.[1].expectedRevision).toBe(1);
		expect(applyAction.mock.calls[1]?.[1].expectedRevision).toBe(2);
		expect(
			applyAction.mock.calls.map(
				([, request]) => request.expectedCaptureModeRevision,
			),
		).toEqual([1, 1]);
	});

	it("refuses writes until exact capture authority is ready", async () => {
		const { store, applyAction, onError, writer } = harness(false);
		const projection = store.getSnapshot().projection;

		await expect(
			writer.setFixtureValue(fixtureInput("not-ready", 0.8)),
		).resolves.toBeNull();

		expect(applyAction).not.toHaveBeenCalled();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: expect.stringMatching(/capture mode is unavailable/i),
			}),
		);
	});

	it.each([
		[
			"loading",
			(store: ProgrammerValuesStore) => store.setLoading(),
			/still loading/i,
		],
		[
			"repair",
			(store: ProgrammerValuesStore) =>
				store.setRepairRequired(new Error("event gap")),
			/being repaired/i,
		],
	] as const)("refuses optimism while values authority is %s", async (_label, makeUnavailable, message) => {
		const { store, applyAction, onError, writer } = harness();
		const projection = store.getSnapshot().projection;
		makeUnavailable(store);

		await expect(writer.clear("values-not-ready")).resolves.toBeNull();

		expect(applyAction).not.toHaveBeenCalled();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: expect.stringMatching(message) }),
		);
	});

	it("refuses normal writes while Preload captures the Programmer", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		captureModeStore.applyProjection(
			captureModeProjection({
				revision: 2,
				blind: true,
				preloadCaptureProgrammer: true,
			}),
			11,
		);
		const projection = store.getSnapshot().projection;

		await expect(writer.clear("preload-active")).resolves.toBeNull();

		expect(applyAction).not.toHaveBeenCalled();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("accepts an in-flight write when capture mode changes after dispatch", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const response = deferred<ProgrammerValuesActionOutcome>();
		const projection = valuesProjection({
			revision: 2,
			fixtureValues: [fixtureValue(0.8)],
		});
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("capture-flip", 0.8));
		await Promise.resolve();

		captureModeStore.applyProjection(
			captureModeProjection({
				revision: 2,
				blind: true,
				preloadCaptureProgrammer: true,
			}),
			11,
		);
		response.resolve(changed("capture-flip", projection));

		await expect(pending).resolves.toMatchObject({ status: "changed" });
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		expect(applyAction).toHaveBeenCalledOnce();
		await expect(writer.clear("after-flip")).resolves.toBeNull();
		expect(applyAction).toHaveBeenCalledOnce();
	});

	it("rolls back a queued write when capture mode changes before dispatch", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const firstResponse = deferred<ProgrammerValuesActionOutcome>();
		applyAction.mockReturnValueOnce(firstResponse.promise);
		const first = writer.setFixtureValue(fixtureInput("first", 0.8));
		const queued = writer.clear("queued");
		await Promise.resolve();

		captureModeStore.applyProjection(
			captureModeProjection({ revision: 2, preview: true }),
			11,
		);
		firstResponse.resolve(
			changed(
				"first",
				valuesProjection({
					revision: 2,
					fixtureValues: [fixtureValue(0.8)],
				}),
			),
		);

		await expect(first).resolves.toMatchObject({ status: "changed" });
		await expect(queued).resolves.toBeNull();
		expect(applyAction).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		expect(fixtureLevel(store)).toBe(0.8);
	});

	it.each([
		["loading", (store: ProgrammerValuesStore) => store.setLoading()],
		[
			"repair",
			(store: ProgrammerValuesStore) =>
				store.setRepairRequired(new Error("event gap")),
		],
	] as const)("does not dispatch a queued write while values authority is %s", async (_label, makeUnavailable) => {
		const { store, applyAction, writer } = harness();
		const firstResponse = deferred<ProgrammerValuesActionOutcome>();
		applyAction.mockReturnValueOnce(firstResponse.promise);
		const first = writer.setFixtureValue(fixtureInput("first", 0.8));
		const queued = writer.clear("queued-values-state");
		await Promise.resolve();
		makeUnavailable(store);
		firstResponse.resolve(
			changed(
				"first",
				valuesProjection({
					revision: 2,
					fixtureValues: [fixtureValue(0.8)],
				}),
			),
		);

		await expect(first).resolves.toMatchObject({ status: "changed" });
		await expect(queued).resolves.toBeNull();
		expect(applyAction).toHaveBeenCalledOnce();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("repairs both authorities and rolls back a revision conflict", async () => {
		const {
			store,
			captureModeStore,
			applyAction,
			repair,
			repairCaptureMode,
			writer,
		} = harness();
		const conflict = Object.assign(new Error("revision conflict"), {
			status: 409,
			currentCaptureModeRevision: 2,
		});
		applyAction.mockRejectedValueOnce(conflict);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(valuesSnapshot({ cursor: 12, revision: 2 }));
		});
		repairCaptureMode.mockImplementationOnce(async () => {
			captureModeStore.installRepairSnapshot(
				captureModeSnapshot({ cursor: 12, revision: 2 }),
			);
		});

		await writer.setFixtureValue(fixtureInput("conflict", 0.8));
		expect(repair).toHaveBeenCalledOnce();
		expect(repairCaptureMode).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		expect(captureModeStore.getSnapshot().projection?.revision).toBe(2);
	});

	it("drops late responses after either authority scope is replaced", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const lateCapture = deferred<ProgrammerValuesActionOutcome>();
		applyAction.mockReturnValueOnce(lateCapture.promise);
		const pendingCapture = writer.clear("late-capture");
		await Promise.resolve();
		captureModeStore.reset(SHOW_ID, USER_ID, "session-b");
		lateCapture.resolve(noChange("late-capture"));
		await expect(pendingCapture).resolves.toBeNull();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);

		const late = deferred<ProgrammerValuesActionOutcome>();
		const next = harness();
		next.applyAction.mockReturnValueOnce(late.promise);
		const pending = next.writer.clear("late-values");
		await Promise.resolve();
		next.store.reset(SHOW_ID, USER_ID, "session-b");
		late.resolve(noChange("late-values", 2));
		await expect(pending).resolves.toBeNull();
		expect(next.store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
		});
	});

	it("suppresses a conflict after deferred repairs outlive both scopes", async () => {
		const {
			store,
			captureModeStore,
			applyAction,
			repair,
			repairCaptureMode,
			onError,
			writer,
		} = harness();
		const valuesRepair = deferred<void>();
		const captureRepair = deferred<void>();
		applyAction.mockRejectedValueOnce(
			Object.assign(new Error("revision conflict"), { status: 409 }),
		);
		repair.mockReturnValueOnce(valuesRepair.promise);
		repairCaptureMode.mockReturnValueOnce(captureRepair.promise);
		const pending = writer.setFixtureValue(fixtureInput("late-conflict", 0.8));
		await Promise.resolve();
		await Promise.resolve();
		expect(repair).toHaveBeenCalledOnce();
		expect(repairCaptureMode).toHaveBeenCalledOnce();

		writer.stop();
		store.reset(SHOW_ID, USER_ID, "session-b");
		captureModeStore.reset(SHOW_ID, USER_ID, "session-b");
		valuesRepair.resolve();
		captureRepair.resolve();
		await expect(pending).resolves.toBeNull();
		await Promise.resolve();
		await Promise.resolve();

		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
			error: null,
		});
		expect(captureModeStore.getSnapshot()).toMatchObject({
			projection: null,
			error: null,
		});
	});

	it("removes active and queued optimism when the writer stops", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const active = writer.setFixtureValue(fixtureInput("active", 0.8));
		const queued = writer.clear("queued-stop");
		await Promise.resolve();
		expect(store.getSnapshot().pendingRequestIds).toEqual([
			"active",
			"queued-stop",
		]);

		writer.stop();

		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		await expect(active).resolves.toBeNull();
		await expect(queued).resolves.toBeNull();
		response.resolve(noChange("active"));
		await Promise.resolve();
	});
});
