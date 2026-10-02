import { describe, expect, it, vi } from "vitest";
import { encodeProgrammerPreloadValuesActionRequest } from "../../api/programmerPreloadValuesWire";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
} from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
} from "./contracts";
import { ProgrammerPreloadValuesStore } from "./store";
import {
	FIXTURE_1,
	OTHER_SESSION_ID,
	SESSION_ID,
	SHOW_ID,
	preloadProjection,
	preloadSnapshot,
} from "./testFixtures";
import { ProgrammerPreloadValuesWriter } from "./writer";

type ApplyAction = (
	scope: { showId: string },
	request: ProgrammerPreloadValuesActionRequest,
) => Promise<ProgrammerPreloadValuesActionOutcome>;

function harness() {
	const store = new ProgrammerPreloadValuesStore();
	store.reset(SHOW_ID, SESSION_ID, "session-a");
	store.installSnapshot(preloadSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	captureModeStore.installSnapshot(
		captureModeSnapshot({ blind: true, preloadCaptureProgrammer: true }),
	);
	const applyAction = vi.fn<ApplyAction>();
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

function outcome(
	requestId: string,
	overrides: Partial<ProgrammerPreloadValuesActionOutcome> = {},
): ProgrammerPreloadValuesActionOutcome {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change",
		preloadRevision: 1,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
		...overrides,
	} as ProgrammerPreloadValuesActionOutcome;
}

function changed(requestId: string, revision: number) {
	return outcome(requestId, {
		status: "changed",
		preloadRevision: revision,
		projection: preloadProjection({ revision }),
		eventSequence: 20,
	} as Partial<ProgrammerPreloadValuesActionOutcome>);
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((complete, fail) => {
		resolve = complete;
		reject = fail;
	});
	return { promise, resolve, reject };
}

function intent(requestId: string, undoGroup: string | null, level = 0.5) {
	return {
		requestId,
		fixtureIds: [FIXTURE_1],
		attribute: "intensity",
		operation: {
			type: "absolute_set" as const,
			value: { kind: "normalized" as const, value: level },
		},
		undoGroup,
		timing: { fade: false, fadeMillis: null, delayMillis: null },
	};
}

function fixture(requestId: string) {
	return {
		requestId,
		fixtureId: FIXTURE_1,
		attribute: "intensity",
		value: { kind: "normalized" as const, value: 0.4 },
		fade: false,
		fadeMillis: null,
		delayMillis: null,
	};
}

function finish(requestId: string, undoGroup: string) {
	return { requestId, attribute: "intensity", undoGroup };
}

async function flush() {
	for (let turn = 0; turn < 10; turn++)
		await new Promise((resolve) => setTimeout(resolve, 0));
}

function sent(applyAction: ReturnType<typeof harness>["applyAction"]) {
	return applyAction.mock.calls.map((call) => call[1].requestId);
}

function sentRequest(
	applyAction: ReturnType<typeof harness>["applyAction"],
	requestId: string,
) {
	return applyAction.mock.calls.find(
		(call) => call[1].requestId === requestId,
	)?.[1];
}

function errors(onError: ReturnType<typeof vi.fn>) {
	return onError.mock.calls
		.map((call) => call[0])
		.filter((error): error is Error => error instanceof Error);
}

describe("ProgrammerPreloadValuesWriter gesture finish ordering", () => {
	it("settles the in-flight edit, then exactly one Finish, then the fresh touch", async () => {
		const { store, applyAction, onError, repair, writer } = harness();
		const edit = deferred<ProgrammerPreloadValuesActionOutcome>();
		const finishResponse = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(edit.promise)
			.mockImplementationOnce(async (_scope, request) =>
				outcome(request.requestId, { preloadRevision: 2 }),
			)
			.mockReturnValueOnce(finishResponse.promise)
			.mockImplementation(async (_scope, request) =>
				outcome(request.requestId, { preloadRevision: 2 }),
			);
		const inFlight = writer.applyIntent(intent("a-1", "gesture-a"));
		const unsent = writer.applyIntent(intent("a-2", "gesture-a", 0.7));
		const barrier = writer.setFixtureValue(fixture("barrier"));
		await flush();
		expect(sent(applyAction)).toEqual(["a-1"]);

		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		const duplicate = writer.finishGesture(finish("finish-a-2", "gesture-a"));
		const fresh = writer.applyIntent(intent("b-1", "gesture-b", 0.9));
		await expect(unsent).resolves.toBeNull();
		await expect(duplicate).resolves.toBeNull();
		expect(store.getSnapshot().pendingRequestIds).toEqual([
			"a-1",
			"barrier",
			"b-1",
		]);
		await flush();
		expect(sent(applyAction)).toEqual(["a-1"]);

		edit.resolve(changed("a-1", 2));
		await expect(inFlight).resolves.toMatchObject({ status: "changed" });
		await expect(barrier).resolves.toMatchObject({ requestId: "barrier" });
		await flush();
		expect(sent(applyAction)).toEqual(["a-1", "barrier", "finish-a"]);

		const request = sentRequest(applyAction, "finish-a");
		expect(request).toEqual({
			requestId: "finish-a",
			expectedPreloadRevision: 2,
			expectedCaptureModeRevision: 1,
			action: {
				action: "finish_gesture",
				attribute: "intensity",
				undoGroup: "gesture-a",
			},
		});
		expect(encodeProgrammerPreloadValuesActionRequest(request!)).toEqual({
			request_id: "finish-a",
			expected_revision: 2,
			expected_capture_mode_revision: 1,
			action: {
				type: "finish_gesture",
				attribute: "intensity",
				undo_group: "gesture-a",
			},
		});

		finishResponse.resolve(outcome("finish-a", { preloadRevision: 2 }));
		await expect(finished).resolves.toMatchObject({
			requestId: "finish-a",
			status: "no_change",
		});
		await expect(fresh).resolves.toMatchObject({ requestId: "b-1" });
		expect(sent(applyAction)).toEqual(["a-1", "barrier", "finish-a", "b-1"]);
		expect(sentRequest(applyAction, "b-1")?.action).toMatchObject({
			action: "apply_intent",
			undoGroup: "gesture-b",
		});
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			error: null,
		});
		expect(repair).not.toHaveBeenCalled();
		expect(errors(onError)).toEqual([]);
	});

	it("keeps other gestures' unsent rows in order around the Finish", async () => {
		const { applyAction, writer } = harness();
		const edit = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(edit.promise)
			.mockImplementation(async (_scope, request) =>
				outcome(request.requestId),
			);
		void writer.applyIntent(intent("a-1", "gesture-a"));
		const other = writer.applyIntent(intent("c-1", "gesture-c"));
		void writer.applyIntent(intent("a-2", "gesture-a"));
		await flush();
		void writer.finishGesture(finish("finish-a", "gesture-a"));
		edit.resolve(outcome("a-1"));
		await expect(other).resolves.toMatchObject({ requestId: "c-1" });
		await flush();
		expect(sent(applyAction)).toEqual(["a-1", "c-1", "finish-a"]);
	});
});

describe("ProgrammerPreloadValuesWriter gesture finish responses", () => {
	it("accepts a delayed stale end with current revisions and no optimistic authoring", async () => {
		const { store, applyAction, onError, repair, repairCaptureMode, writer } =
			harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const projection = store.getSnapshot().projection;
		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		await flush();
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
		expect(store.getSnapshot().projection).toBe(projection);

		response.resolve(
			outcome("finish-a", { preloadRevision: 9, captureModeRevision: 7 }),
		);
		await expect(finished).resolves.toMatchObject({
			status: "no_change",
			preloadRevision: 9,
			captureModeRevision: 7,
		});
		expect(store.getSnapshot().projection).toBe(projection);
		expect(store.getSnapshot()).toMatchObject({ error: null });
		expect(repair).not.toHaveBeenCalled();
		expect(repairCaptureMode).not.toHaveBeenCalled();
		expect(errors(onError)).toEqual([]);
	});

	it("accepts an exact replay and rejects a mismatched or authoring response visibly", async () => {
		const { store, applyAction, onError, writer } = harness();
		applyAction
			.mockResolvedValueOnce(
				outcome("finish-a", {
					preloadRevision: 0,
					captureModeRevision: 0,
					replayed: true,
				}),
			)
			.mockResolvedValueOnce(outcome("someone-else"))
			.mockResolvedValueOnce(changed("finish-c", 2));
		const projection = store.getSnapshot().projection;

		await expect(
			writer.finishGesture(finish("finish-a", "gesture-a")),
		).resolves.toMatchObject({ replayed: true, preloadRevision: 0 });
		await expect(
			writer.finishGesture(finish("finish-b", "gesture-b")),
		).resolves.toBeNull();
		await expect(
			writer.finishGesture(finish("finish-c", "gesture-c")),
		).resolves.toBeNull();

		expect(errors(onError).map((error) => error.message)).toEqual([
			expect.stringMatching(/request identity does not match/),
			expect.stringMatching(/must not change values/),
		]);
		expect(store.getSnapshot().projection).toBe(projection);
	});

	it("sends a Finish after capture mode changes and after Preload is disarmed", async () => {
		const { captureModeStore, applyAction, onError, writer } = harness();
		const edit = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(edit.promise)
			.mockImplementation(async (_scope, request) =>
				outcome(request.requestId, { captureModeRevision: 2 }),
			);
		void writer.applyIntent(intent("a-1", "gesture-a"));
		const queuedEdit = writer.applyIntent(intent("b-1", "gesture-b"));
		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		await flush();

		captureModeStore.applyProjection(
			captureModeProjection({ revision: 2 }),
			11,
		);
		edit.resolve(outcome("a-1"));

		await expect(queuedEdit).resolves.toBeNull();
		await expect(finished).resolves.toMatchObject({
			requestId: "finish-a",
			captureModeRevision: 2,
		});
		expect(sent(applyAction)).toEqual(["a-1", "finish-a"]);
		expect(sentRequest(applyAction, "finish-a")).toMatchObject({
			expectedCaptureModeRevision: 2,
		});
		expect(errors(onError).map((error) => error.message)).toEqual([
			"Programmer capture mode changed before the Preload write was sent",
		]);

		await expect(
			writer.finishGesture(finish("finish-c", "gesture-c")),
		).resolves.toMatchObject({ requestId: "finish-c" });
		await expect(writer.setFixtureValue(fixture("refused"))).resolves.toBeNull();
		expect(sent(applyAction)).toEqual(["a-1", "finish-a", "finish-c"]);
	});
});

describe("ProgrammerPreloadValuesWriter gesture finish scope", () => {
	it("abandons a queued Finish quietly when the session scope is replaced", async () => {
		const { store, applyAction, onError, writer } = harness();
		const edit = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(edit.promise);
		void writer.applyIntent(intent("a-1", "gesture-a"));
		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		await flush();

		store.reset(SHOW_ID, OTHER_SESSION_ID, "session-b");
		store.installSnapshot(preloadSnapshot());
		edit.resolve(outcome("a-1"));

		await expect(finished).resolves.toBeNull();
		await flush();
		expect(sent(applyAction)).toEqual(["a-1"]);
		expect(errors(onError)).toEqual([]);
		await expect(
			writer.finishGesture(finish("finish-late", "gesture-a")),
		).resolves.toBeNull();
		expect(sent(applyAction)).toEqual(["a-1"]);
	});

	it("drops a late Finish response or failure after the desk scope is replaced", async () => {
		const { captureModeStore, applyAction, onError, writer } = harness();
		const first = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(first.promise);
		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		await flush();
		expect(sent(applyAction)).toEqual(["finish-a"]);

		captureModeStore.reset(SHOW_ID, SESSION_ID, "desk-b");
		first.reject(new TypeError("Failed to fetch"));

		await expect(finished).resolves.toBeNull();
		expect(errors(onError)).toEqual([]);
	});
});

describe("ProgrammerPreloadValuesWriter gesture finish identity and disposal", () => {
	it("needs a fresh request ID and leaves a fresh touch's gesture alone", async () => {
		const { applyAction, onError, writer } = harness();
		const edit = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction
			.mockReturnValueOnce(edit.promise)
			.mockImplementation(async (_scope, request) =>
				outcome(request.requestId),
			);
		void writer.applyIntent(intent("a-1", "gesture-a"));
		const queued = writer.applyIntent(intent("a2-1", "gesture-a2"));
		await expect(
			writer.finishGesture(finish("a2-1", "gesture-a")),
		).resolves.toBeNull();
		expect(errors(onError).map((error) => error.message)).toEqual([
			"Preload Programmer values request a2-1 is already pending",
		]);
		await expect(
			writer.finishGesture({ requestId: "", attribute: "x", undoGroup: "g" }),
		).resolves.toBeNull();

		const finished = writer.finishGesture(finish("finish-a", "gesture-a"));
		edit.resolve(outcome("a-1"));
		await expect(queued).resolves.toMatchObject({ requestId: "a2-1" });
		await expect(finished).resolves.toMatchObject({ requestId: "finish-a" });
		await expect(
			writer.finishGesture(finish("finish-a2", "gesture-a2")),
		).resolves.toMatchObject({ requestId: "finish-a2" });
		expect(sent(applyAction)).toEqual(["a-1", "a2-1", "finish-a", "finish-a2"]);
	});

	it("never sends after the writer is stopped and ignores a late in-flight reply", async () => {
		const { applyAction, onError, writer } = harness();
		const response = deferred<ProgrammerPreloadValuesActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const inFlight = writer.finishGesture(finish("finish-a", "gesture-a"));
		const queued = writer.finishGesture(finish("finish-b", "gesture-b"));
		await flush();

		writer.stop();
		await expect(queued).resolves.toBeNull();
		response.resolve(outcome("finish-a"));
		await expect(inFlight).resolves.toBeNull();
		await expect(
			writer.finishGesture(finish("finish-c", "gesture-c")),
		).resolves.toBeNull();
		await flush();
		expect(sent(applyAction)).toEqual(["finish-a"]);
		expect(errors(onError)).toEqual([]);
	});

	it("reports a genuine transport failure without repair or a store change", async () => {
		const { store, applyAction, onError, repair, writer } = harness();
		applyAction
			.mockRejectedValueOnce(new TypeError("Failed to fetch"))
			.mockImplementation(async (_scope, request) =>
				outcome(request.requestId),
			);
		const projection = store.getSnapshot().projection;

		await expect(
			writer.finishGesture(finish("finish-a", "gesture-a")),
		).resolves.toBeNull();
		expect(errors(onError).map((error) => error.message)).toEqual([
			"Failed to fetch",
		]);
		expect(repair).not.toHaveBeenCalled();
		expect(store.getSnapshot().projection).toBe(projection);
		await expect(writer.setFixtureValue(fixture("after-failure"))).resolves.toMatchObject({
			requestId: "after-failure",
		});
	});
});
