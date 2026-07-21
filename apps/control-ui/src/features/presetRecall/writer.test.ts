import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { ProgrammerValuesStore } from "../programmerValues/store";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	PresetRecallOutcome,
	PresetRecallRequest,
	PresetRecallTransport,
} from "./contracts";
import { PresetRecallTransportError } from "./contracts";
import { PresetRecallWriter } from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";

function preset() {
	return {
		kind: "preset" as const,
		id: "2.7",
		revision: 4,
		updated_at: "",
		body: {
			name: "Deep Blue",
			number: 7,
			family: "Color" as const,
			values: {},
		},
	};
}

function valuesProjection(revision = 6, level = 0.25) {
	return {
		userId: USER_ID,
		revision,
		fixtureValues: [
			{
				fixtureId: FIXTURE_ID,
				attribute: "intensity",
				value: { kind: "normalized" as const, value: level },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		],
		groupValues: [],
	};
}

function outcome(
	request: PresetRecallRequest,
	overrides: Partial<PresetRecallOutcome> = {},
): PresetRecallOutcome {
	return {
		requestId: request.requestId,
		correlationId: CORRELATION_ID,
		replayed: false,
		showRevision: 12,
		programmerRevision: 7,
		captureModeRevision: 3,
		selectionRevision: 8,
		interactionEventSequence: null,
		appliedFixtures: 1,
		activeContext: "preset:2.7",
		preset: preset(),
		warning: null,
		status: "changed",
		projection: valuesProjection(7, 0.8),
		eventSequence: 41,
		...overrides,
	} as PresetRecallOutcome;
}

function sparseOutcome(
	request: PresetRecallRequest,
	overrides: Partial<PresetRecallOutcome> = {},
): PresetRecallOutcome {
	return outcome(request, {
		programmerRevision: 6,
		status: "changed",
		projection: null,
		eventSequence: null,
		...overrides,
	});
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

function currentCommand(store: ProgrammingInteractionStore) {
	const commandLine = store.getSnapshot().commandLine;
	if (!commandLine) throw new Error("Missing command-line test authority");
	return commandLine;
}

function harness(recallImplementation?: PresetRecallTransport["recall"]) {
	const showStore = new ShowObjectsStore();
	showStore.reset(SHOW_ID, "session-a");
	showStore.setCollection(SHOW_ID, "preset", [preset()], 30, 12);
	const valuesStore = new ProgrammerValuesStore();
	valuesStore.reset(SHOW_ID, USER_ID, "session-a");
	valuesStore.installSnapshot({ cursor: 30, projection: valuesProjection() });
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, USER_ID, "session-a");
	captureModeStore.installSnapshot({
		cursor: 30,
		projection: {
			userId: USER_ID,
			revision: 3,
			blind: false,
			preview: false,
			preloadCaptureProgrammer: false,
		},
	});
	const programmingStore = new ProgrammingInteractionStore();
	programmingStore.reset(SHOW_ID, DESK_ID, "session-a");
	programmingStore.installSnapshot({
		cursor: 30,
		projection: {
			deskId: DESK_ID,
			commandLine: {
				text: "FIXTURE",
				target: "FIXTURE",
				pristine: true,
				revision: 1,
				pendingChoice: null,
			},
			selection: {
				selected: [FIXTURE_ID],
				expression: { type: "static" },
				revision: 8,
				gestureOpen: false,
			},
		},
	});
	const recall = vi.fn(
		recallImplementation ?? (async (_scope, request) => outcome(request)),
	);
	const repairValues = vi.fn(async () => undefined);
	const repairCaptureMode = vi.fn(async () => undefined);
	const repairSelection = vi.fn(async () => undefined);
	const loadPreset = vi.fn(async () => ({
		object: preset(),
		showRevision: 12,
	}));
	const onError = vi.fn();
	const writer = new PresetRecallWriter({
		scope: { showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
		showStore,
		valuesStore,
		captureModeStore,
		programmingStore,
		transport: { recall },
		loadPreset,
		repairValues,
		repairCaptureMode,
		repairSelection,
		onError,
	});
	return {
		writer,
		showStore,
		valuesStore,
		captureModeStore,
		programmingStore,
		recall,
		repairValues,
		repairCaptureMode,
		repairSelection,
		loadPreset,
		onError,
	};
}

const input = {
	objectId: "2.7",
	address: { family: "Color" as const, number: 7 },
};

describe("PresetRecallWriter", () => {
	it("captures every exact authority at interaction time without optimistic expansion", async () => {
		const pending = deferred<PresetRecallOutcome>();
		const setup = harness((_scope, _request) => pending.promise);

		const recalled = setup.writer.recall(input);
		await Promise.resolve();

		expect(setup.recall).toHaveBeenCalledOnce();
		const [scope, request] = setup.recall.mock.calls[0];
		expect(scope).toEqual({
			showId: SHOW_ID,
			userId: USER_ID,
			deskId: DESK_ID,
		});
		expect(request).toMatchObject({
			presetId: "2.7",
			address: { family: "Color", number: 7 },
			expectedPresetRevision: 4,
			expectedShowRevision: 12,
			expectedProgrammerRevision: 6,
			expectedCaptureModeRevision: 3,
			expectedSelectionRevision: 8,
			selectedFixtureCount: 1,
		});
		expect(setup.valuesStore.getSnapshot().projection?.revision).toBe(6);
		pending.resolve(outcome(request));
		await expect(recalled).resolves.toMatchObject({ status: "changed" });
		expect(setup.valuesStore.getSnapshot()).toMatchObject({
			eventSequence: 41,
			projection: { revision: 7 },
		});
	});

	it("reconciles response-before-event and event-before-response idempotently", async () => {
		const responseFirst = harness();
		const first = await responseFirst.writer.recall(input);
		expect(first?.status).toBe("changed");
		expect(() =>
			responseFirst.valuesStore.applyProjection(valuesProjection(7, 0.8), 41),
		).not.toThrow();
		expect(responseFirst.repairValues).not.toHaveBeenCalled();

		const pending = deferred<PresetRecallOutcome>();
		const eventFirst = harness((_scope, _request) => pending.promise);
		const recalled = eventFirst.writer.recall(input);
		await Promise.resolve();
		const request = eventFirst.recall.mock.calls[0][1];
		eventFirst.valuesStore.applyProjection(valuesProjection(7, 0.8), 41);
		pending.resolve(outcome(request));
		await expect(recalled).resolves.toMatchObject({ status: "changed" });
		expect(eventFirst.valuesStore.getSnapshot().projection?.revision).toBe(7);
		expect(eventFirst.repairValues).not.toHaveBeenCalled();
	});

	it("repairs a response-first gesture close and accepts an event-first close", async () => {
		const responseFirst = harness();
		responseFirst.repairSelection.mockImplementation(async () => {
			responseFirst.programmingStore.installSnapshot({
				cursor: 42,
				projection: {
					deskId: DESK_ID,
					commandLine: currentCommand(responseFirst.programmingStore),
					selection: {
						selected: [FIXTURE_ID],
						expression: { type: "static" },
						revision: 9,
						gestureOpen: false,
					},
				},
			});
		});
		responseFirst.recall.mockImplementation(async (_scope, request) =>
			sparseOutcome(request, {
				selectionRevision: 9,
				interactionEventSequence: 42,
			}),
		);

		await expect(responseFirst.writer.recall(input)).resolves.toMatchObject({
			interactionEventSequence: 42,
		});
		expect(responseFirst.repairSelection).toHaveBeenCalledOnce();
		expect(
			responseFirst.programmingStore.getSnapshot().selection,
		).toMatchObject({
			revision: 9,
			gestureOpen: false,
		});

		const pending = deferred<PresetRecallOutcome>();
		const eventFirst = harness((_scope, _request) => pending.promise);
		const recalled = eventFirst.writer.recall(input);
		await Promise.resolve();
		const request = eventFirst.recall.mock.calls[0][1];
		eventFirst.programmingStore.applyChange(
			{
				deskId: DESK_ID,
				selection: {
					selected: [FIXTURE_ID],
					expression: { type: "static" },
					revision: 9,
					gestureOpen: false,
				},
			},
			42,
		);
		pending.resolve(
			sparseOutcome(request, {
				selectionRevision: 9,
				interactionEventSequence: 42,
			}),
		);
		await recalled;
		expect(eventFirst.repairSelection).not.toHaveBeenCalled();
	});

	it("accepts a replay of an already-observed gesture close without repairing", async () => {
		let attempt = 0;
		let setup!: ReturnType<typeof harness>;
		setup = harness(async (_scope, request) => {
			attempt++;
			if (attempt === 1) {
				setup.programmingStore.applyChange(
					{
						deskId: DESK_ID,
						selection: {
							selected: [FIXTURE_ID],
							expression: { type: "static" },
							revision: 9,
							gestureOpen: false,
						},
					},
					42,
				);
				throw new PresetRecallTransportError(
					"response lost",
					"unavailable",
					0,
					null,
					null,
					true,
				);
			}
			return sparseOutcome(request, {
				replayed: true,
				selectionRevision: 9,
				interactionEventSequence: 42,
			});
		});

		await expect(setup.writer.recall(input)).resolves.toMatchObject({
			replayed: true,
			interactionEventSequence: 42,
		});
		expect(setup.recall).toHaveBeenCalledTimes(2);
		expect(setup.recall.mock.calls[0][1].requestId).toBe(
			setup.recall.mock.calls[1][1].requestId,
		);
		expect(setup.repairSelection).not.toHaveBeenCalled();
	});

	it("handles replay, context-only changed, and no-change without materializing values", async () => {
		const setup = harness();
		setup.recall.mockImplementationOnce(async (_scope, request) =>
			sparseOutcome(request, { replayed: true }),
		);
		const contextOnly = await setup.writer.recall(input);
		expect(contextOnly).toMatchObject({
			status: "changed",
			replayed: true,
			projection: null,
		});
		expect(setup.valuesStore.getSnapshot().projection?.revision).toBe(6);

		setup.recall.mockImplementationOnce(async (_scope, request) =>
			sparseOutcome(request, { status: "no_change" }),
		);
		const noChange = await setup.writer.recall(input);
		expect(noChange?.status).toBe("no_change");
		expect(setup.valuesStore.getSnapshot().projection?.revision).toBe(6);
	});

	it("retries once with the same request ID and rolls back by leaving authority untouched", async () => {
		let attempt = 0;
		const setup = harness(async (_scope, request) => {
			attempt++;
			if (attempt === 1)
				throw new PresetRecallTransportError(
					"response lost",
					"unavailable",
					0,
					null,
					null,
					true,
				);
			return outcome(request, { replayed: true });
		});

		await expect(setup.writer.recall(input)).resolves.toMatchObject({
			replayed: true,
		});
		expect(setup.recall).toHaveBeenCalledTimes(2);
		expect(setup.recall.mock.calls[0][1].requestId).toBe(
			setup.recall.mock.calls[1][1].requestId,
		);

		const failed = harness(async () => {
			throw new PresetRecallTransportError(
				"forbidden",
				"forbidden",
				403,
				null,
				null,
				false,
			);
		});
		await expect(failed.writer.recall(input)).resolves.toBeNull();
		expect(failed.valuesStore.getSnapshot().projection?.revision).toBe(6);
		expect(failed.onError).toHaveBeenLastCalledWith(
			expect.objectContaining({ message: "forbidden" }),
		);
	});

	it("refuses a concurrent click instead of reordering captured authority", async () => {
		const pending = deferred<PresetRecallOutcome>();
		const setup = harness((_scope, _request) => pending.promise);
		const first = setup.writer.recall(input);
		await Promise.resolve();

		await expect(setup.writer.recall(input)).resolves.toBeNull();
		expect(setup.recall).toHaveBeenCalledOnce();
		expect(setup.onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "A Preset recall is already in progress",
			}),
		);
		const request = setup.recall.mock.calls[0][1];
		pending.resolve(outcome(request));
		await expect(first).resolves.toMatchObject({ status: "changed" });
	});

	it("refuses missing, pending, redirected, and empty-selection authority", async () => {
		const loading = harness();
		loading.showStore.markCollectionDormant("preset");
		await expect(loading.writer.recall(input)).resolves.toBeNull();
		expect(loading.recall).not.toHaveBeenCalled();

		const redirected = harness();
		redirected.captureModeStore.applyProjection(
			{
				userId: USER_ID,
				revision: 4,
				blind: true,
				preview: false,
				preloadCaptureProgrammer: true,
			},
			31,
		);
		await expect(redirected.writer.recall(input)).resolves.toBeNull();
		expect(redirected.recall).not.toHaveBeenCalled();

		const empty = harness();
		empty.programmingStore.installSnapshot({
			cursor: 31,
			projection: {
				deskId: DESK_ID,
				commandLine: currentCommand(empty.programmingStore),
				selection: {
					selected: [],
					expression: { type: "static" },
					revision: 9,
					gestureOpen: false,
				},
			},
		});
		await expect(empty.writer.recall(input)).resolves.toBeNull();
		expect(empty.recall).not.toHaveBeenCalled();
	});

	it("drops late outcomes after session, Show, and writer replacement", async () => {
		const pending = deferred<PresetRecallOutcome>();
		const setup = harness((_scope, _request) => pending.promise);
		const recalled = setup.writer.recall(input);
		await Promise.resolve();
		const request = setup.recall.mock.calls[0][1];
		setup.valuesStore.reset(SHOW_ID, USER_ID, "session-b");
		pending.resolve(outcome(request));

		await expect(recalled).resolves.toBeNull();
		expect(setup.valuesStore.getSnapshot().projection).toBeNull();
		expect(setup.onError).not.toHaveBeenCalled();

		const showPending = deferred<PresetRecallOutcome>();
		const showReplacement = harness((_scope, _request) => showPending.promise);
		const showRecall = showReplacement.writer.recall(input);
		await Promise.resolve();
		const showRequest = showReplacement.recall.mock.calls[0][1];
		showReplacement.showStore.reset(
			"99999999-9999-4999-8999-999999999999",
			"session-b",
		);
		showPending.resolve(outcome(showRequest));
		await expect(showRecall).resolves.toBeNull();

		const writerPending = deferred<PresetRecallOutcome>();
		const writerReplacement = harness(
			(_scope, _request) => writerPending.promise,
		);
		const writerRecall = writerReplacement.writer.recall(input);
		await Promise.resolve();
		const writerRequest = writerReplacement.recall.mock.calls[0][1];
		writerReplacement.writer.stop();
		writerPending.resolve(outcome(writerRequest));
		await expect(writerRecall).resolves.toBeNull();
	});

	it("repairs every narrow authority after a revision conflict", async () => {
		const setup = harness(async () => {
			throw new PresetRecallTransportError(
				"revision conflict",
				"conflict",
				409,
				7,
				9,
				false,
			);
		});

		await expect(setup.writer.recall(input)).resolves.toBeNull();

		expect(setup.repairValues).toHaveBeenCalledOnce();
		expect(setup.repairCaptureMode).toHaveBeenCalledOnce();
		expect(setup.repairSelection).toHaveBeenCalledOnce();
		expect(setup.loadPreset).toHaveBeenCalledWith(SHOW_ID, "2.7");
		expect(setup.onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "revision conflict" }),
		);
	});
});
