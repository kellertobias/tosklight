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
	OTHER_USER_ID,
	preloadFixtureValue,
	preloadProjection,
	preloadSnapshot,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";
import { ProgrammerPreloadValuesWriter } from "./writer";

function harness(
	options: { captureReady?: boolean; captureActive?: boolean } = {},
) {
	const store = new ProgrammerPreloadValuesStore();
	store.reset(SHOW_ID, USER_ID, "session-a");
	store.installSnapshot(preloadSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, USER_ID, "session-a");
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
				scope: { showId: string; userId: string },
				request: ProgrammerPreloadValuesActionRequest,
			) => Promise<ProgrammerPreloadValuesActionOutcome>
		>();
	const repair = vi.fn<(error: Error) => Promise<void>>(async () => undefined);
	const repairCaptureMode = vi.fn<(error: Error) => Promise<void>>(
		async () => undefined,
	);
	const onError = vi.fn();
	const writer = new ProgrammerPreloadValuesWriter({
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

	it("replays one ambiguous request with the identical request object", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(new Error("connection reset"))
			.mockResolvedValueOnce(noChange("replay"));

		await writer.releaseFixtureValue({
			requestId: "replay",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
		});

		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
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

	it("rejects a foreign-user action outcome and repairs both authorities", async () => {
		const { store, applyAction, repair, repairCaptureMode, writer } = harness();
		applyAction.mockResolvedValueOnce(
			changed(
				"foreign",
				preloadProjection({ userId: OTHER_USER_ID, revision: 2 }),
			),
		);

		await expect(
			writer.setFixtureValue(fixtureInput("foreign", 0.8)),
		).resolves.toBeNull();

		expect(repair).toHaveBeenCalledOnce();
		expect(repairCaptureMode).toHaveBeenCalledOnce();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(fixtureLevel(store)).toBe(0.25);
	});

	it("drops a late response after either scoped authority is replaced", async () => {
		const { store, captureModeStore, applyAction, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setFixtureValue(fixtureInput("late", 0.8));
		await Promise.resolve();

		store.reset(SHOW_ID, USER_ID, "session-b");
		captureModeStore.reset(SHOW_ID, USER_ID, "session-b");
		response.resolve(noChange("late", 2));

		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			projection: null,
			pendingRequestIds: [],
			error: null,
		});
	});
});
