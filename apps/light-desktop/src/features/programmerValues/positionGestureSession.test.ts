import { describe, expect, it, vi } from "vitest";
import { encodeProgrammerPreloadValuesActionRequest } from "../../api/programmerPreloadValuesWire";
import { encodeProgrammerValuesActionRequest } from "../../api/programmerValuesWire";
import type { PositionCancelReason } from "../../components/modals/specialDialogs/intention/PositionDialog";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { captureModeSnapshot } from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
} from "../programmerPreloadValues/contracts";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { preloadSnapshot } from "../programmerPreloadValues/testFixtures";
import { ProgrammerPreloadValuesWriter } from "../programmerPreloadValues/writer";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "./contracts";
import {
	attachPositionGestureWindowGuards,
	type PositionGestureCancelReason,
	type PositionGestureFinishInput,
	type PositionGestureIntentInput,
	type PositionGestureLane,
	PositionGestureSession,
	type PositionGestureTimers,
	type PositionGestureWriter,
	positionAngleSet,
	positionAngleStep,
} from "./positionGestureSession";
import { ProgrammerValuesStore } from "./store";
import {
	FIXTURE_1,
	OTHER_SESSION_ID,
	SESSION_ID,
	SHOW_ID,
	valuesSnapshot,
} from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };

// Compile-time contracts: the dialog's cancel reasons map onto the session's, and both
// mounted lane writers satisfy the session's writer surface without a pass-through.
const dialogReasonsFit = (reason: PositionCancelReason): PositionGestureCancelReason => reason;
const normalWriterFits = (writer: ProgrammerValuesWriter): PositionGestureWriter => writer;
const preloadWriterFits = (writer: ProgrammerPreloadValuesWriter): PositionGestureWriter => writer;
void dialogReasonsFit;
void normalWriterFits;
void preloadWriterFits;

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

/** Writers report `null` to clear an error; only `Error` values are failures. */
function errors(onError: ReturnType<typeof vi.fn>) {
	return onError.mock.calls
		.map((call) => call[0])
		.filter((error): error is Error => error instanceof Error);
}

async function flush() {
	for (let turn = 0; turn < 10; turn++)
		await new Promise((resolve) => setTimeout(resolve, 0));
}

/** Deterministic, UUID-shaped identities: undo groups and request IDs share one sequence. */
function idSequence() {
	let next = 0;
	return () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`;
}

type LogEntry =
	| { step: "producer-stop"; label: string }
	| { step: "apply"; writer: string; input: PositionGestureIntentInput }
	| { step: "cancel"; writer: string; undoGroup: string }
	| { step: "finish"; writer: string; input: PositionGestureFinishInput };

function fakeWriter(name: string, log: LogEntry[]) {
	const finishResponse = deferred<unknown>();
	const writer: PositionGestureWriter = {
		applyIntent: vi.fn(async (input: PositionGestureIntentInput) => {
			log.push({ step: "apply", writer: name, input });
			return { requestId: input.requestId };
		}),
		cancelGesture: vi.fn((undoGroup: string) => {
			log.push({ step: "cancel", writer: name, undoGroup });
			return 0;
		}),
		finishGesture: vi.fn((input: PositionGestureFinishInput) => {
			log.push({ step: "finish", writer: name, input });
			return finishResponse.promise;
		}),
	};
	return { writer, finishResponse };
}

function fakeHarness(timers?: PositionGestureTimers) {
	const log: LogEntry[] = [];
	const normal = fakeWriter("normal", log);
	const preload = fakeWriter("preload", log);
	let lane: PositionGestureLane = "normal";
	const onError = vi.fn();
	const session = new PositionGestureSession({
		writerFor: (requested) =>
			requested === "normal" ? normal.writer : preload.writer,
		createId: idSequence(),
		timers,
		onError,
	});
	const start = (stopProducer?: () => void, idleEndMillis?: number) =>
		session.start({
			lane,
			fixtureIds: [FIXTURE_1],
			timing: TIMING,
			stopProducer:
				stopProducer ??
				(() => log.push({ step: "producer-stop", label: "producer" })),
			idleEndMillis,
		});
	return {
		log,
		normal,
		preload,
		session,
		onError,
		start,
		switchLane: (next: PositionGestureLane) => {
			lane = next;
		},
	};
}

const steps = (log: LogEntry[]) => log.map((entry) => entry.step);
const finishes = (log: LogEntry[]) =>
	log.filter(
		(entry): entry is Extract<LogEntry, { step: "finish" }> =>
			entry.step === "finish",
	);

describe("PositionGestureSession terminal ordering", () => {
	const reasons: PositionGestureCancelReason[] = [
		"pointer-cancel",
		"lost-capture",
		"blur",
		"hidden",
		"close",
		"superseded",
		"teardown",
	];

	it("release stops the producer, then cancels unsent rows, then sends one Finish", async () => {
		const { log, normal, start, onError } = fakeHarness();
		const gesture = start()!;
		await gesture.change({ pan: positionAngleStep(4) });
		expect(gesture.end()).toBe(true);
		expect(steps(log)).toEqual(["apply", "producer-stop", "cancel", "finish"]);
		expect(log[2]).toEqual({
			step: "cancel",
			writer: "normal",
			undoGroup: gesture.undoGroup,
		});
		expect(finishes(log)[0]?.input).toEqual({
			requestId: expect.any(String),
			attribute: "position",
			undoGroup: gesture.undoGroup,
		});
		expect(gesture.endReason).toBe("release");
		normal.finishResponse.resolve({ status: "no_change" });
		await expect(gesture.finished).resolves.toEqual({ status: "no_change" });
		expect(onError).not.toHaveBeenCalled();
	});

	it.each(reasons)("cancel(%s) uses the same ordered terminal path", (reason) => {
		const { log, start } = fakeHarness();
		const gesture = start()!;
		expect(gesture.cancel(reason)).toBe(true);
		expect(steps(log)).toEqual(["producer-stop", "cancel", "finish"]);
		expect(gesture.endReason).toBe(reason);
		expect(gesture.isOpen).toBe(false);
	});

	it("runs every registered producer stop before cleanup, in registration order", () => {
		const { log, start } = fakeHarness();
		const gesture = start()!;
		gesture.addProducerStop(() => log.push({ step: "producer-stop", label: "frames" }));
		const removed = gesture.addProducerStop(() =>
			log.push({ step: "producer-stop", label: "removed" }),
		);
		removed();
		gesture.cancel("pointer-cancel");
		expect(log.slice(0, 2)).toEqual([
			{ step: "producer-stop", label: "producer" },
			{ step: "producer-stop", label: "frames" },
		]);
		expect(steps(log).slice(2)).toEqual(["cancel", "finish"]);
		// A late registration on an ended gesture stops at once.
		const late = vi.fn();
		gesture.addProducerStop(late);
		expect(late).toHaveBeenCalledOnce();
	});

	it("sends exactly one Finish however many terminal paths fire", () => {
		const { log, session, start } = fakeHarness();
		const gesture = start()!;
		expect(gesture.end()).toBe(true);
		expect(gesture.end()).toBe(false);
		expect(gesture.cancel("blur")).toBe(false);
		expect(session.cancel("hidden")).toBe(false);
		session.dispose();
		expect(finishes(log)).toHaveLength(1);
		expect(steps(log).filter((step) => step === "producer-stop")).toHaveLength(1);
		expect(steps(log).filter((step) => step === "cancel")).toHaveLength(1);
		expect(gesture.endReason).toBe("release");
	});

	it("refuses a change after the end", async () => {
		const { log, normal, start } = fakeHarness();
		const gesture = start()!;
		gesture.end();
		expect(gesture.change({ pan: positionAngleSet(10) })).toBeNull();
		await flush();
		expect(normal.writer.applyIntent).not.toHaveBeenCalled();
		expect(steps(log)).toEqual(["producer-stop", "cancel", "finish"]);
	});
});

describe("PositionGestureSession identity and lanes", () => {
	it("mints a fresh Undo group per gesture and a fresh request ID per change and Finish", async () => {
		const { log, start } = fakeHarness();
		const first = start()!;
		await first.change({ pan: positionAngleStep(1) });
		await first.change({ tilt: positionAngleStep(1) });
		first.end();
		const second = start()!;
		await second.change({ pan: positionAngleStep(1) });
		second.cancel("lost-capture");
		expect(first.undoGroup).not.toBe(second.undoGroup);
		const applies = log.filter(
			(entry): entry is Extract<LogEntry, { step: "apply" }> =>
				entry.step === "apply",
		);
		expect(applies.map((entry) => entry.input.undoGroup)).toEqual([
			first.undoGroup,
			first.undoGroup,
			second.undoGroup,
		]);
		const [finishA, finishB] = finishes(log).map((entry) => entry.input);
		expect(finishA?.undoGroup).toBe(first.undoGroup);
		expect(finishB?.undoGroup).toBe(second.undoGroup);
		const ids = [
			first.undoGroup,
			second.undoGroup,
			...applies.map((entry) => entry.input.requestId),
			finishA?.requestId,
			finishB?.requestId,
		];
		expect(new Set(ids).size).toBe(ids.length);
	});

	it("submits Position component edits built from the TL-618 contract", async () => {
		const { log, start } = fakeHarness();
		const gesture = start()!;
		await gesture.change({
			activateAngles: true,
			pan: positionAngleSet(-270),
			tilt: positionAngleStep(2.5),
		});
		expect(log[0]).toEqual({
			step: "apply",
			writer: "normal",
			input: {
				requestId: expect.any(String),
				fixtureIds: [FIXTURE_1],
				groupId: null,
				attribute: "position",
				operation: {
					type: "component_edits",
					edits: [
						{ kind: "activate_angles" },
						{
							kind: "scalar",
							component: { kind: "pan" },
							operation: { kind: "set", value: { kind: "value", value: -270 } },
						},
						{
							kind: "scalar",
							component: { kind: "tilt" },
							operation: { kind: "relative", value: 2.5 },
						},
					],
				},
				undoGroup: gesture.undoGroup,
				timing: TIMING,
			},
		});
	});

	it("refuses an empty or invalid change without sending", async () => {
		const { normal, onError, start } = fakeHarness();
		const gesture = start()!;
		expect(gesture.change({})).toBeNull();
		expect(gesture.change({ pan: positionAngleStep(Number.NaN) })).toBeNull();
		expect(onError).toHaveBeenCalledOnce();
		expect(normal.writer.applyIntent).not.toHaveBeenCalled();
		expect(gesture.isOpen).toBe(true);
	});

	it("keeps the lane pinned at start when the lane switches mid-gesture", async () => {
		const { log, normal, preload, start, switchLane } = fakeHarness();
		const gesture = start()!;
		switchLane("preload");
		await gesture.change({ pan: positionAngleStep(3) });
		gesture.end();
		expect(gesture.lane).toBe("normal");
		expect(log.every((entry) => !("writer" in entry) || entry.writer === "normal")).toBe(true);
		expect(preload.writer.applyIntent).not.toHaveBeenCalled();
		expect(preload.writer.finishGesture).not.toHaveBeenCalled();
		expect(normal.writer.finishGesture).toHaveBeenCalledOnce();

		const next = start()!;
		expect(next.lane).toBe("preload");
		await next.change({ tilt: positionAngleStep(1) });
		next.end();
		expect(preload.writer.finishGesture).toHaveBeenCalledOnce();
	});

	it("supersedes an open gesture before a new start", () => {
		const { log, start } = fakeHarness();
		const first = start()!;
		const second = start()!;
		expect(first.endReason).toBe("superseded");
		expect(second.isOpen).toBe(true);
		expect(finishes(log).map((entry) => entry.input.undoGroup)).toEqual([
			first.undoGroup,
		]);
	});
});

describe("PositionGestureSession surface lifecycle", () => {
	function guards() {
		const window = new EventTarget() as Window;
		const document = Object.assign(new EventTarget(), { hidden: false }) as Document & {
			hidden: boolean;
		};
		return { window, document };
	}

	it("ends exactly once on window blur", () => {
		const { log, session, start } = fakeHarness();
		const target = guards();
		const detach = attachPositionGestureWindowGuards(session, target);
		const gesture = start()!;
		target.window.dispatchEvent(new Event("blur"));
		target.window.dispatchEvent(new Event("blur"));
		expect(gesture.endReason).toBe("blur");
		expect(finishes(log)).toHaveLength(1);
		detach();
	});

	it("ends exactly once when the document becomes hidden, not when it becomes visible", () => {
		const { log, session, start } = fakeHarness();
		const target = guards();
		const detach = attachPositionGestureWindowGuards(session, target);
		const gesture = start()!;
		target.document.dispatchEvent(new Event("visibilitychange"));
		expect(gesture.isOpen).toBe(true);
		target.document.hidden = true;
		target.document.dispatchEvent(new Event("visibilitychange"));
		target.document.dispatchEvent(new Event("visibilitychange"));
		expect(gesture.endReason).toBe("hidden");
		expect(finishes(log)).toHaveLength(1);
		detach();
		const later = start()!;
		target.window.dispatchEvent(new Event("blur"));
		expect(later.isOpen).toBe(true);
	});

	it("dispose ends an open gesture once with teardown and refuses later starts", () => {
		const { log, session, start } = fakeHarness();
		const gesture = start()!;
		session.dispose();
		session.dispose();
		expect(gesture.endReason).toBe("teardown");
		expect(steps(log)).toEqual(["producer-stop", "cancel", "finish"]);
		expect(start()).toBeNull();
		expect(session.active).toBeNull();
	});
});

describe("PositionGestureSession idle-ending encoder mode", () => {
	function manualTimers() {
		const pending = new Map<number, { callback: () => void; at: number }>();
		let now = 0;
		let next = 0;
		const timers: PositionGestureTimers = {
			setTimeout: (callback, millis) => {
				pending.set(++next, { callback, at: now + millis });
				return next;
			},
			clearTimeout: (handle) => {
				pending.delete(handle as number);
			},
		};
		const advance = (millis: number) => {
			now += millis;
			for (const [handle, timer] of [...pending]) {
				if (timer.at > now) continue;
				pending.delete(handle);
				timer.callback();
			}
		};
		return { timers, advance, pending };
	}

	it("ends once after the idle timeout and resets on every change", async () => {
		const clock = manualTimers();
		const { log, start } = fakeHarness(clock.timers);
		const gesture = start(undefined, 500)!;
		clock.advance(400);
		await gesture.change({ pan: positionAngleStep(1) });
		clock.advance(400);
		expect(gesture.isOpen).toBe(true);
		await gesture.change({ tilt: positionAngleStep(-1) });
		clock.advance(499);
		expect(gesture.isOpen).toBe(true);
		clock.advance(1);
		expect(gesture.endReason).toBe("idle");
		// Detents are discrete steps: the idle end keeps admitted edits (no cancel) and asks the
		// writer to queue the Finish behind them.
		expect(steps(log)).toEqual(["apply", "apply", "producer-stop", "finish"]);
		expect(finishes(log)[0]?.input).toMatchObject({ keepAdmittedEdits: true });
		clock.advance(5000);
		expect(finishes(log)).toHaveLength(1);
		expect(clock.pending.size).toBe(0);
		expect(gesture.change({ pan: positionAngleStep(1) })).toBeNull();
	});

	it("clears the idle timer on an explicit end or dispose", () => {
		const clock = manualTimers();
		const { log, session, start } = fakeHarness(clock.timers);
		start(undefined, 500)!.end();
		expect(clock.pending.size).toBe(0);
		start(undefined, 500);
		session.dispose();
		expect(clock.pending.size).toBe(0);
		clock.advance(1000);
		expect(finishes(log)).toHaveLength(2);
	});

	it("a pointer gesture without an idle timeout never ends on time", () => {
		const clock = manualTimers();
		const { start } = fakeHarness(clock.timers);
		const gesture = start()!;
		expect(clock.pending.size).toBe(0);
		expect(gesture.isOpen).toBe(true);
	});
});

function normalHarness() {
	const store = new ProgrammerValuesStore();
	store.reset(SHOW_ID, SESSION_ID, "session-a");
	store.installSnapshot(valuesSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	captureModeStore.installSnapshot(captureModeSnapshot());
	const applyAction =
		vi.fn<(scope: { showId: string }, request: ProgrammerValuesActionRequest) => Promise<ProgrammerValuesActionOutcome>>();
	const writerError = vi.fn();
	const writer = new ProgrammerValuesWriter({
		scope: { showId: SHOW_ID },
		store,
		captureModeStore,
		applyAction,
		repair: async () => undefined,
		repairCaptureMode: async () => undefined,
		onError: writerError,
	});
	return { store, applyAction, writerError, writer };
}

function preloadHarness() {
	const store = new ProgrammerPreloadValuesStore();
	store.reset(SHOW_ID, SESSION_ID, "session-a");
	store.installSnapshot(preloadSnapshot());
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	captureModeStore.installSnapshot(
		captureModeSnapshot({ blind: true, preloadCaptureProgrammer: true }),
	);
	const applyAction =
		vi.fn<(scope: { showId: string }, request: ProgrammerPreloadValuesActionRequest) => Promise<ProgrammerPreloadValuesActionOutcome>>();
	const writerError = vi.fn();
	const writer = new ProgrammerPreloadValuesWriter({
		scope: { showId: SHOW_ID },
		store,
		captureModeStore,
		applyAction,
		repair: async () => undefined,
		repairCaptureMode: async () => undefined,
		onError: writerError,
	});
	return { store, applyAction, writerError, writer };
}

function noChange(requestId: string) {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change" as const,
		revision: 1,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
	};
}

describe("PositionGestureSession with the mounted lane writers", () => {
	it("Normal: the in-flight edit completes, the unsent edit drops and one Finish follows", async () => {
		const normal = normalHarness();
		const preload = preloadHarness();
		const onError = vi.fn();
		const session = new PositionGestureSession({
			writerFor: (lane) => (lane === "normal" ? normal.writer : preload.writer),
			createId: idSequence(),
			onError,
		});
		const edit = deferred<ProgrammerValuesActionOutcome>();
		normal.applyAction
			.mockReturnValueOnce(edit.promise)
			.mockImplementation(async (_scope, request) => noChange(request.requestId));
		const producer = vi.fn();
		const gesture = session.start({
			lane: "normal",
			fixtureIds: [FIXTURE_1],
			timing: TIMING,
			stopProducer: producer,
		})!;
		const first = gesture.change({ pan: positionAngleStep(5) })!;
		const unsent = gesture.change({ pan: positionAngleStep(5) })!;
		await flush();
		expect(normal.applyAction).toHaveBeenCalledTimes(1);
		gesture.end();
		expect(producer).toHaveBeenCalledOnce();
		await expect(unsent).resolves.toBeNull();
		edit.resolve(noChange(normal.applyAction.mock.calls[0]![1].requestId));
		await expect(first).resolves.toMatchObject({ status: "no_change" });
		await expect(gesture.finished).resolves.toMatchObject({ status: "no_change" });
		const requests = normal.applyAction.mock.calls.map((call) => call[1]);
		expect(requests.map((request) => request.action.action)).toEqual([
			"apply_intent",
			"finish_gesture",
		]);
		expect(encodeProgrammerValuesActionRequest(requests[0]!).action).toMatchObject({
			type: "apply_intent",
			attribute: "position",
			undo_group: gesture.undoGroup,
			operation: {
				type: "component_edits",
				edits: [{ kind: "scalar", component: { kind: "pan" }, operation: { kind: "relative", value: 5 } }],
			},
		});
		expect(encodeProgrammerValuesActionRequest(requests[1]!).action).toEqual({
			type: "finish_gesture",
			attribute: "position",
			undo_group: gesture.undoGroup,
		});
		expect(preload.applyAction).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(errors(normal.writerError)).toEqual([]);
	});

	it("Preload: the Preload lane edits and finishes through the Preload writer only", async () => {
		const normal = normalHarness();
		const preload = preloadHarness();
		const session = new PositionGestureSession({
			writerFor: (lane) => (lane === "normal" ? normal.writer : preload.writer),
			createId: idSequence(),
		});
		preload.applyAction.mockImplementation(async (_scope, request) => ({
			...noChange(request.requestId),
			revision: undefined,
			preloadRevision: 1,
		}) as unknown as ProgrammerPreloadValuesActionOutcome);
		const gesture = session.start({ lane: "preload", fixtureIds: [FIXTURE_1], timing: TIMING })!;
		await gesture.change({ tilt: positionAngleSet(45) });
		gesture.cancel("close");
		await gesture.finished;
		const requests = preload.applyAction.mock.calls.map((call) => call[1]);
		expect(requests.map((request) => request.action.action)).toEqual([
			"apply_intent",
			"finish_gesture",
		]);
		expect(encodeProgrammerPreloadValuesActionRequest(requests[1]!).action).toEqual({
			type: "finish_gesture",
			attribute: "position",
			undo_group: gesture.undoGroup,
		});
		expect(requests[0]!.requestId).not.toBe(requests[1]!.requestId);
		expect(normal.applyAction).not.toHaveBeenCalled();
		expect(errors(preload.writerError)).toEqual([]);
	});

	it("a disposed writer is quiet: changes and the Finish resolve null without errors", async () => {
		const normal = normalHarness();
		const onError = vi.fn();
		const session = new PositionGestureSession({
			writerFor: () => normal.writer,
			createId: idSequence(),
			onError,
		});
		const gesture = session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!;
		normal.writer.stop();
		await expect(gesture.change({ pan: positionAngleStep(1) })).resolves.toBeNull();
		expect(gesture.end()).toBe(true);
		await expect(gesture.finished).resolves.toBeNull();
		expect(normal.applyAction).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(errors(normal.writerError)).toEqual([]);
	});

	it("a gone scope is quiet: the Finish is abandoned and a missing lane writer refuses the start", async () => {
		const normal = normalHarness();
		const onError = vi.fn();
		let writer: PositionGestureWriter | null = normal.writer;
		const session = new PositionGestureSession({
			writerFor: () => writer,
			createId: idSequence(),
			onError,
		});
		normal.applyAction.mockImplementation(async (_scope, request) => noChange(request.requestId));
		const gesture = session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!;
		await gesture.change({ pan: positionAngleStep(1) });
		normal.store.reset(SHOW_ID, OTHER_SESSION_ID, "session-b");
		gesture.cancel("teardown");
		await expect(gesture.finished).resolves.toBeNull();
		expect(normal.applyAction).toHaveBeenCalledTimes(1);
		writer = null;
		expect(session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })).toBeNull();
		expect(onError).not.toHaveBeenCalled();
		expect(errors(normal.writerError)).toEqual([]);
	});

	it("a writer that rejects reports once and still settles the gesture", async () => {
		const onError = vi.fn();
		const writer: PositionGestureWriter = {
			applyIntent: async () => {
				throw new TypeError("Failed to fetch");
			},
			cancelGesture: () => 0,
			finishGesture: async () => {
				throw new TypeError("Failed to fetch");
			},
		};
		const session = new PositionGestureSession({ writerFor: () => writer, createId: idSequence(), onError });
		const gesture = session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!;
		await expect(gesture.change({ pan: positionAngleStep(1) })).resolves.toBeNull();
		gesture.end();
		await expect(gesture.finished).resolves.toBeNull();
		expect(onError).toHaveBeenCalledTimes(2);
	});
});
