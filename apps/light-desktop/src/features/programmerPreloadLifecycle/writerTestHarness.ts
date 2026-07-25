import { vi } from "vitest";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import {
	deskProjection,
	playbackSnapshot,
	SHOW_ID,
} from "../playbackRuntime/testFixtures";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import { ProgrammerPreloadPlaybackQueueStore } from "../programmerPreloadPlaybackQueue/store";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	ProgrammerPreloadLifecycleOutcome,
	ProgrammerPreloadLifecycleRequest,
	ProgrammerPreloadLifecycleTransport,
} from "./contracts";
import { ProgrammerPreloadLifecycleStore } from "./store";
import { ProgrammerPreloadLifecycleWriter } from "./writer";

export { SHOW_ID };
export const USER_ID = "33333333-3333-4333-8333-333333333333";
export const DESK_ID = deskProjection().desk_id;
export const OTHER_ID = "99999999-9999-4999-8999-999999999999";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const PROGRAMMER_ID = "66666666-6666-4666-8666-666666666666";

export function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((complete, fail) => {
		resolve = complete;
		reject = fail;
	});
	return { promise, resolve, reject };
}

export function captureMode(revision = 1, blind = false) {
	return {
		userId: USER_ID,
		revision,
		blind,
		preview: false,
		preloadCaptureProgrammer: blind,
	};
}

export function values(revision = 1, empty = false) {
	return {
		userId: USER_ID,
		revision,
		fixtureValues: empty
			? []
			: [
					{
						fixtureId: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized" as const, value: 0.5 },
						programmerOrder: 1,
						fade: false,
						fadeMillis: null,
						delayMillis: null,
					},
				],
		groupValues: [],
	};
}

export function queue(revision = 2, empty = false) {
	return {
		userId: USER_ID,
		revision,
		actions: empty
			? []
			: [
					{
						playbackNumber: 9,
						page: null,
						action: "go" as const,
						surface: "physical" as const,
					},
				],
	};
}

export function lifecycleRow(preloadActive = false) {
	return {
		programmerId: PROGRAMMER_ID,
		userId: USER_ID,
		connected: true,
		selectedFixtureCount: 1,
		normalValueCount: 1,
		preloadActive,
		sessions: [],
	};
}

export function outcome(
	request: ProgrammerPreloadLifecycleRequest,
	overrides: Partial<ProgrammerPreloadLifecycleOutcome> = {},
): ProgrammerPreloadLifecycleOutcome {
	return {
		requestId: request.requestId,
		correlationId: CORRELATION_ID,
		replayed: false,
		status: "no_change",
		active: false,
		captureMode: captureMode(),
		captureModeEventSequence: null,
		valuesRevision: 1,
		valuesProjection: null,
		valuesEventSequence: null,
		queueRevision: 2,
		queueProjection: null,
		queueEventSequence: null,
		interactionEventSequence: null,
		selectionRevision: 1,
		commit: null,
		warning: null,
		...overrides,
	};
}

export function goOutcome(request: ProgrammerPreloadLifecycleRequest) {
	return outcome(request, {
		status: "changed",
		active: true,
		captureMode: captureMode(2),
		captureModeEventSequence: 31,
		valuesRevision: 2,
		valuesProjection: values(2, true),
		valuesEventSequence: 32,
		queueRevision: 3,
		queueProjection: queue(3, true),
		queueEventSequence: 33,
		commit: {
			showId: SHOW_ID,
			showRevision: 4,
			playbackEventSequenceBefore: 30,
			playbackEventSequenceAfter: 30,
			committedAt: "2026-07-21T10:00:00Z",
			programmerFadeMillis: 2_000,
			executedPlaybackActions: 1,
			executed: queue().actions,
			runtimeChanges: [],
		},
	});
}

export function lifecycleWriterHarness(
	applyAction?: ProgrammerPreloadLifecycleTransport["applyAction"],
	options: { blind?: boolean; active?: boolean | null } = {},
) {
	const localStore = new ProgrammerPreloadLifecycleStore();
	localStore.reset(SHOW_ID, USER_ID, DESK_ID, "session-a");
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, USER_ID, "session-a");
	captureModeStore.installSnapshot({
		cursor: 10,
		projection: captureMode(1, options.blind ?? false),
	});
	const valuesStore = new ProgrammerPreloadValuesStore();
	valuesStore.reset(SHOW_ID, USER_ID, "session-a");
	valuesStore.installSnapshot({ cursor: 11, projection: values() });
	const queueStore = new ProgrammerPreloadPlaybackQueueStore();
	queueStore.reset(SHOW_ID, USER_ID, "session-a");
	queueStore.installSnapshot({ cursor: 12, projection: queue() });
	const selectionStore = new ProgrammingInteractionStore();
	selectionStore.reset(SHOW_ID, DESK_ID, "session-a");
	selectionStore.installSnapshot({
		cursor: 13,
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
				revision: 1,
				gestureOpen: false,
			},
		},
	});
	const lifecycleStore = new ProgrammerLifecycleStore();
	lifecycleStore.reset("session-a");
	if (options.active !== null)
		lifecycleStore.installSnapshot({
			cursor: 14,
			projection: {
				revision: 1,
				programmers: [lifecycleRow(options.active ?? false)],
			},
		});
	const showStore = new ShowObjectsStore();
	showStore.reset(SHOW_ID, "session-a");
	const runtimeStore = new PlaybackRuntimeStore();
	runtimeStore.reset(SHOW_ID, DESK_ID, "session-a");
	runtimeStore.installSnapshot(playbackSnapshot([], 30), []);
	const readActive = () => {
		const state = lifecycleStore.getSnapshot();
		if (state.status !== "ready" || state.repairRequired) return null;
		return (
			state.projection?.programmers.find((row) => row.userId === USER_ID)
				?.preloadActive ?? null
		);
	};
	const setActive = (preloadActive: boolean) => {
		const state = lifecycleStore.getSnapshot();
		if (!state.projection || state.eventSequence === null)
			throw new Error("Missing lifecycle test authority");
		lifecycleStore.applyChange(
			{
				revision: state.projection.revision + 1,
				delta: { type: "upsert", programmer: lifecycleRow(preloadActive) },
			},
			state.eventSequence + 1,
		);
	};
	const apply = vi.fn(
		applyAction ?? (async (_scope, request) => outcome(request)),
	);
	const repair = {
		captureMode: vi.fn(async () => undefined),
		values: vi.fn(async () => undefined),
		queue: vi.fn(async () => undefined),
		selection: vi.fn(async () => undefined),
		lifecycle: vi.fn(async () => undefined),
		runtime: vi.fn(async () => undefined),
	};
	const onError = vi.fn();
	const writer = new ProgrammerPreloadLifecycleWriter({
		scope: { showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
		store: localStore,
		captureModeStore,
		valuesStore,
		queueStore,
		selectionStore,
		lifecycleStore,
		showStore,
		runtimeStore,
		readPreloadActive: readActive,
		transport: { applyAction: apply },
		repair,
		onError,
	});
	return {
		writer,
		localStore,
		captureModeStore,
		valuesStore,
		queueStore,
		selectionStore,
		lifecycleStore,
		showStore,
		runtimeStore,
		readActive,
		setActive,
		apply,
		repair,
		onError,
	};
}
