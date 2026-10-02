import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
} from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesProjection,
} from "./contracts";
import { ProgrammerPreloadValuesStore } from "./store";
import {
	FIXTURE_1,
	OTHER_SESSION_ID,
	preloadFixtureValue,
	preloadProjection,
	preloadSnapshot,
	SHOW_ID,
	SESSION_ID,
} from "./testFixtures";
import { ProgrammerPreloadValuesWriter } from "./writer";

function harness(
	options: { captureReady?: boolean; captureActive?: boolean } = {},
) {
	const store = new ProgrammerPreloadValuesStore();
	store.reset(SHOW_ID, SESSION_ID, "session-a");
	store.installSnapshot(preloadSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	if (options.captureReady !== false)
		captureModeStore.installSnapshot(
			captureModeSnapshot({
				blind: options.captureActive !== false,
				preloadCaptureProgrammer: options.captureActive !== false,
			}),
		);
	const applyAction =
		vi.fn<
			(
				scope: { showId: string },
				request: ProgrammerPreloadValuesActionRequest,
			) => Promise<ProgrammerPreloadValuesActionOutcome>
		>();
	const repair = vi.fn<(error: Error) => Promise<void>>(async () => undefined);
	const repairCaptureMode = vi.fn<(error: Error) => Promise<void>>(
		async () => undefined,
	);
	const onError = vi.fn();
	const writer = new ProgrammerPreloadValuesWriter({
		scope: { showId: SHOW_ID },
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
	projection: ProgrammerPreloadValuesProjection,
	eventSequence = 20,
): ProgrammerPreloadValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "changed",
		preloadRevision: projection.revision,
		captureModeRevision: 1,
		projection,
		eventSequence,
		replayed: false,
		warning: null,
	};
}

function noChange(
	requestId: string,
	preloadRevision = 1,
): ProgrammerPreloadValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change",
		preloadRevision,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
	};
}

function fixtureInput(requestId: string, level: number) {
	return {
		requestId,
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: { kind: "normalized" as const, value: level },
		fade: true,
		fadeMillis: 500,
		delayMillis: 100,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function fixtureLevel(store: ProgrammerPreloadValuesStore) {
	const value = store.getSnapshot().projection?.fixtureValues[0]?.value;
	return value?.kind === "normalized" ? value.value : null;
}

describe("ProgrammerPreloadValuesWriter contract", () => {
	it("sends typed fixture/group set and release actions with both revisions", async () => {
		const { applyAction, writer } = harness();
		applyAction.mockResolvedValue(noChange("unused"));
		applyAction.mockImplementation(async (_scope, request) =>
			noChange(request.requestId),
		);

		await writer.setFixtureValue(fixtureInput("fixture-set", 0.8));
		await writer.releaseFixtureValue({
			requestId: "fixture-release",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});
		await writer.setGroupValue({
			requestId: "group-set",
			groupId: "front",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.6 },
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		});
		await writer.releaseGroupValue({
			requestId: "group-release",
			groupId: "front",
			attribute: "intensity",
		});

		expect(applyAction.mock.calls.map(([, request]) => request.action)).toEqual(
			[
				expect.objectContaining({
					action: "set_fixture",
					timing: { fade: true, fadeMillis: 500, delayMillis: 100 },
				}),
				expect.objectContaining({ action: "release_fixture" }),
				expect.objectContaining({ action: "set_group" }),
				expect.objectContaining({ action: "release_group" }),
			],
		);
		expect(
			applyAction.mock.calls.map(([, request]) => ({
				preload: request.expectedPreloadRevision,
				capture: request.expectedCaptureModeRevision,
			})),
		).toEqual([
			{ preload: 1, capture: 1 },
			{ preload: 1, capture: 1 },
			{ preload: 1, capture: 1 },
			{ preload: 1, capture: 1 },
		]);
	});

	it("sends an ordered batch as one application action and one request", async () => {
		const { applyAction, writer } = harness();
		applyAction.mockResolvedValueOnce(noChange("batch-a"));
		const mutations = [
			{
				action: "set_fixture" as const,
				fixtureId: FIXTURE_1,
				attribute: "intensity",
				value: { kind: "normalized" as const, value: 0.8 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
			{
				action: "release_group" as const,
				groupId: "front",
				attribute: "intensity",
			},
		];

		await writer.batch({ requestId: "batch-a", mutations });

		expect(applyAction).toHaveBeenCalledOnce();
		expect(applyAction.mock.calls[0]?.[1].action).toEqual({
			action: "batch",
			mutations,
		});
	});

	it("settles response-before-event without republishing the duplicate", async () => {
		const { store, applyAction, writer } = harness();
		const projection = preloadProjection({
			revision: 2,
			fixtureValues: [preloadFixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockResolvedValueOnce(changed("response-first", projection));

		await writer.setFixtureValue(fixtureInput("response-first", 0.8));
		const settled = store.getSnapshot().projection;
		store.applyProjection(projection, 20);

		expect(store.getSnapshot()).toMatchObject({
			eventSequence: 20,
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
		expect(store.getSnapshot().projection).toBe(settled);
	});

	it("settles event-before-response and removes only matching optimism", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		const projection = preloadProjection({
			revision: 2,
			fixtureValues: [preloadFixtureValue(0.8, { programmerOrder: 3 })],
		});
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("event-first", 0.8));
		await Promise.resolve();

		store.applyProjection(projection, 20);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["event-first"]);
		response.resolve(changed("event-first", projection));
		await pending;

		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(store.getSnapshot().eventSequence).toBe(20);
	});

	it("tracks a no-change request without cloning its projection", async () => {
		const { store, applyAction, writer } = harness();
		const projection = store.getSnapshot().projection;
		applyAction.mockResolvedValueOnce(noChange("same"));

		const pending = writer.setFixtureValue({
			...fixtureInput("same", 0.25),
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		});
		expect(store.getSnapshot().projection).toBe(projection);
		await pending;

		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("repairs one ambiguous request without resending it", async () => {
		const { applyAction, repair, repairCaptureMode, writer } = harness();
		applyAction.mockRejectedValueOnce(new Error("connection reset"));

		await writer.releaseFixtureValue({
			requestId: "replay",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});

		expect(applyAction).toHaveBeenCalledOnce();
		expect(repair).toHaveBeenCalledOnce();
		expect(repairCaptureMode).toHaveBeenCalledOnce();
	});
});

describe("ProgrammerPreloadValuesWriter preconditions and recovery", () => {
	it.each([
		["not ready", { captureReady: false }],
		["inactive", { captureActive: false }],
	] as const)("refuses optimism while capture mode is %s", async (_label, options) => {
		const { store, applyAction, onError, writer } = harness(options);
		const projection = store.getSnapshot().projection;

		await expect(
			writer.setFixtureValue(fixtureInput("refused", 0.8)),
		).resolves.toBeNull();

		expect(applyAction).not.toHaveBeenCalled();
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(onError).toHaveBeenCalledWith(expect.any(Error));
	});

	it("rolls back a rejected mutation without replaying a definitive error", async () => {
		const { store, applyAction, onError, writer } = harness();
		applyAction.mockRejectedValueOnce(
			Object.assign(new Error("invalid value"), { status: 400 }),
		);

		const pending = writer.setFixtureValue(fixtureInput("rejected", 0.8));
		expect(fixtureLevel(store)).toBe(0.8);
		await expect(pending).resolves.toBeNull();

		expect(applyAction).toHaveBeenCalledOnce();
		expect(fixtureLevel(store)).toBe(0.25);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "invalid value" }),
		);
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
		applyAction.mockRejectedValueOnce(
			Object.assign(new Error("revision conflict"), { status: 409 }),
		);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(preloadSnapshot({ cursor: 12, revision: 2 }));
		});
		repairCaptureMode.mockImplementationOnce(async () => {
			captureModeStore.installRepairSnapshot(
				captureModeSnapshot({
					cursor: 12,
					revision: 2,
					blind: true,
					preloadCaptureProgrammer: true,
				}),
			);
		});

		await writer.setFixtureValue(fixtureInput("conflict", 0.8));

		expect(repair).toHaveBeenCalledOnce();
		expect(repairCaptureMode).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
		});
	});

	it("rolls back a queued write if capture mode changes before dispatch", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const firstResponse = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(firstResponse.promise);
		const first = writer.setFixtureValue(fixtureInput("first", 0.8));
		const queued = writer.releaseFixtureValue({
			requestId: "queued",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});
		await Promise.resolve();
		captureModeStore.applyProjection(
			captureModeProjection({
				revision: 2,
				blind: false,
				preloadCaptureProgrammer: false,
			}),
			11,
		);
		firstResponse.resolve(
			changed(
				"first",
				preloadProjection({
					revision: 2,
					fixtureValues: [preloadFixtureValue(0.8)],
				}),
			),
		);

		await expect(first).resolves.toMatchObject({ status: "changed" });
		await expect(queued).resolves.toBeNull();
		expect(applyAction).toHaveBeenCalledOnce();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("drops a late response after either scoped authority is replaced", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("late", 0.8));
		await Promise.resolve();

		store.reset(SHOW_ID, SESSION_ID, "session-b");
		captureModeStore.reset(SHOW_ID, SESSION_ID, "session-b");
		response.resolve(noChange("late", 2));

		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
			error: null,
		});
	});
});

function intentInput(requestId: string, undoGroup: string | null, level = 0.5) {
	return {
		requestId,
		fixtureIds: [FIXTURE_1],
		attribute: "intensity",
		operation: {
			type: "absolute_set" as const,
			value: { kind: "normalized" as const, value: level },
		},
		undoGroup,
		timing: { fade: true, fadeMillis: 500, delayMillis: null },
	};
}

async function flushWrites() {
	for (let turn = 0; turn < 10; turn++)
		await new Promise((resolve) => setTimeout(resolve, 0));
}

function settlementSpy<T>(promise: Promise<T>) {
	const spy = vi.fn();
	void promise.then(spy, spy);
	return spy;
}

function sentRequestIds(
	applyAction: ReturnType<typeof harness>["applyAction"],
) {
	return applyAction.mock.calls.map((call) => call[1].requestId);
}

describe("ProgrammerPreloadValuesWriter gesture cancellation", () => {
	it("drops only unsent rows of the stopped gesture and keeps the dispatched row", async () => {
		const { store, applyAction, onError, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		applyAction.mockImplementation(async (_scope, request) =>
			noChange(request.requestId, request.expectedPreloadRevision),
		);
		const dispatched = writer.applyIntent(intentInput("a-1", "gesture-a"));
		const unsentA = writer.applyIntent(intentInput("a-2", "gesture-a", 0.6));
		const barrier = writer.setFixtureValue(fixtureInput("barrier", 0.7));
		const gestureB = writer.applyIntent(intentInput("b-1", "gesture-b"));
		const unsentA2 = writer.applyIntent(intentInput("a-3", "gesture-a", 0.8));
		await flushWrites();
		expect(sentRequestIds(applyAction)).toEqual(["a-1"]);
		const dispatchedSettled = settlementSpy(dispatched);

		expect(writer.cancelGesture("gesture-a")).toBe(2);

		await expect(unsentA).resolves.toBeNull();
		await expect(unsentA2).resolves.toBeNull();
		expect(store.getSnapshot().pendingRequestIds).toEqual([
			"a-1",
			"barrier",
			"b-1",
		]);
		expect(dispatchedSettled).not.toHaveBeenCalled();

		const projection = preloadProjection({ revision: 2 });
		response.resolve(changed("a-1", projection));
		await expect(dispatched).resolves.toMatchObject({
			requestId: "a-1",
			status: "changed",
		});
		await expect(barrier).resolves.toMatchObject({ requestId: "barrier" });
		await expect(gestureB).resolves.toMatchObject({ requestId: "b-1" });
		expect(dispatchedSettled).toHaveBeenCalledOnce();
		expect(applyAction.mock.calls[0]?.[1]).toMatchObject({
			requestId: "a-1",
			expectedPreloadRevision: 1,
			expectedCaptureModeRevision: 1,
			action: { action: "apply_intent", undoGroup: "gesture-a" },
		});
		expect(sentRequestIds(applyAction)).toEqual(["a-1", "barrier", "b-1"]);
		expect(applyAction.mock.calls[1]?.[1].expectedPreloadRevision).toBe(2);
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { revision: 2 },
			error: null,
		});
		expect(onError).not.toHaveBeenCalledWith(expect.any(Error));
	});

	it("keeps a dispatched row and its genuine error when cancelled during repair", async () => {
		const { store, applyAction, repair, onError, writer } = harness();
		const valuesRepair = deferred<void>();
		applyAction.mockRejectedValueOnce(
			Object.assign(new Error("revision conflict"), { status: 409 }),
		);
		applyAction.mockImplementation(async (_scope, request) =>
			noChange(request.requestId, request.expectedPreloadRevision),
		);
		repair.mockReturnValueOnce(valuesRepair.promise);
		const dispatched = writer.applyIntent(intentInput("a-1", "gesture-a"));
		const unsent = writer.applyIntent(intentInput("a-2", "gesture-a"));
		const gestureB = writer.applyIntent(intentInput("b-1", "gesture-b"));
		await flushWrites();
		expect(repair).toHaveBeenCalledOnce();

		expect(writer.cancelGesture("gesture-a")).toBe(1);
		await expect(unsent).resolves.toBeNull();
		expect(store.getSnapshot().pendingRequestIds).toEqual(["a-1", "b-1"]);

		valuesRepair.resolve();
		await expect(dispatched).resolves.toBeNull();
		await expect(gestureB).resolves.toMatchObject({ requestId: "b-1" });
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "revision conflict" }),
		);
		expect(sentRequestIds(applyAction)).toEqual(["a-1", "b-1"]);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("tolerates repeated cancellation, late completion and a fresh gesture ID", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		applyAction.mockImplementation(async (_scope, request) =>
			noChange(request.requestId, request.expectedPreloadRevision),
		);
		const dispatched = writer.applyIntent(intentInput("a-1", "gesture-a"));
		const unsent = writer.applyIntent(intentInput("a-2", "gesture-a"));
		await flushWrites();
		const unsentSettled = settlementSpy(unsent);

		expect(writer.cancelGesture("gesture-a")).toBe(1);
		expect(writer.cancelGesture("gesture-a")).toBe(0);
		expect(writer.cancelGesture("")).toBe(0);
		await flushWrites();
		expect(unsentSettled).toHaveBeenCalledOnce();
		expect(unsentSettled).toHaveBeenCalledWith(null);

		const fresh = writer.applyIntent(intentInput("a2-1", "gesture-a2"));
		response.resolve(noChange("a-1"));
		await expect(dispatched).resolves.toMatchObject({ requestId: "a-1" });
		await expect(fresh).resolves.toMatchObject({ requestId: "a2-1" });
		expect(writer.cancelGesture("gesture-a")).toBe(0);
		await flushWrites();
		expect(unsentSettled).toHaveBeenCalledOnce();
		expect(sentRequestIds(applyAction)).toEqual(["a-1", "a2-1"]);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("never cancels untagged rows and preserves an existing store error", async () => {
		const { store, applyAction, writer } = harness();
		applyAction.mockRejectedValueOnce(
			Object.assign(new Error("invalid value"), { status: 400 }),
		);
		await writer.setFixtureValue(fixtureInput("rejected", 0.8));
		const genuine = store.getSnapshot().error;
		expect(genuine?.message).toBe("invalid value");

		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		applyAction.mockImplementation(async (_scope, request) =>
			noChange(request.requestId, request.expectedPreloadRevision),
		);
		const dispatched = writer.applyIntent(intentInput("a-1", "gesture-a"));
		const untagged = writer.applyIntent(intentInput("plain", null));
		const unsent = writer.applyIntent(intentInput("a-2", "gesture-a"));
		await flushWrites();

		expect(writer.cancelGesture("gesture-a")).toBe(1);
		await expect(unsent).resolves.toBeNull();
		expect(store.getSnapshot().error).toBe(genuine);
		response.resolve(noChange("a-1"));
		await expect(dispatched).resolves.toMatchObject({ requestId: "a-1" });
		await expect(untagged).resolves.toMatchObject({ requestId: "plain" });
	});

	it("keeps permanent stop semantics after a cancellation", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const dispatched = writer.applyIntent(intentInput("a-1", "gesture-a"));
		const gestureB = writer.applyIntent(intentInput("b-1", "gesture-b"));
		await flushWrites();
		writer.cancelGesture("gesture-a");
		writer.stop();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		await expect(dispatched).resolves.toBeNull();
		await expect(gestureB).resolves.toBeNull();
		expect(writer.cancelGesture("gesture-b")).toBe(0);
		await expect(
			writer.applyIntent(intentInput("after-stop", "gesture-c")),
		).resolves.toBeNull();
		response.resolve(noChange("a-1"));
		await flushWrites();
		expect(applyAction).toHaveBeenCalledOnce();
	});
});
