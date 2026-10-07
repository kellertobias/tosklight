import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { captureModeSnapshot } from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "./contracts";
import { DisplayedSourceReadouts } from "./displayedSource";
import {
	type FamilyGestureIntentInput,
	FamilyGestureSession,
	type FamilyGestureWriter,
} from "./familyGestureSession";
import {
	POSITION_GESTURE_FAMILY,
	positionAngleStep,
} from "./positionGestureSession";
import { ProgrammerValuesStore } from "./store";
import { FIXTURE_1, SESSION_ID, SHOW_ID, valuesSnapshot } from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };

function ids() {
	let next = 0;
	return () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`;
}

function recordingWriter(outcome: unknown = { status: "changed" }) {
	const applied: FamilyGestureIntentInput[] = [];
	const writer: FamilyGestureWriter = {
		applyIntent: vi.fn(async (input: FamilyGestureIntentInput) => {
			applied.push(input);
			return outcome;
		}),
		cancelGesture: vi.fn(() => 0),
		finishGesture: vi.fn(async () => ({ status: "no_change" })),
	};
	return { writer, applied };
}

function readoutsWithLease(lane: "normal" | "preload", lease: number) {
	const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
	readouts.observe({
		lane,
		scope: { show_id: null },
		lease,
		revision: 1,
		owners: [],
	});
	return readouts;
}

describe("FamilyGestureSession displayed source (TL-594)", () => {
	it("names the lease read at start on every edit of that gesture", async () => {
		const { writer, applied } = recordingWriter();
		const readouts = readoutsWithLease("normal", 7);
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
			displayedSource: (lane) => readouts.displayedSource(lane),
		});
		const gesture = session.start({
			lane: "normal",
			fixtureIds: [FIXTURE_1],
			timing: TIMING,
		})!;
		await gesture.change({ pan: positionAngleStep(2) });
		readouts.observe({
			lane: "normal",
			scope: { show_id: null },
			lease: 8,
			revision: 1,
			owners: [],
		});
		await gesture.change({ pan: positionAngleStep(1) });
		expect(applied.map((input) => input.displayedSource)).toEqual([
			{ lane: "normal", lease: 7 },
			{ lane: "normal", lease: 7 },
		]);
		gesture.end();
		const next = session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!;
		await next.change({ pan: positionAngleStep(1) });
		expect(applied[2].displayedSource).toEqual({ lane: "normal", lease: 8 });
	});

	it("omits the source without a provider and never crosses lanes", async () => {
		const { writer, applied } = recordingWriter();
		const plain = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
		});
		await plain
			.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!
			.change({ pan: positionAngleStep(1) });
		expect("displayedSource" in applied[0]).toBe(false);
		const crossed = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
			displayedSource: () => ({ lane: "normal", lease: 3 }),
		});
		await crossed
			.start({ lane: "preload", fixtureIds: [FIXTURE_1], timing: TIMING })!
			.change({ pan: positionAngleStep(1) });
		expect("displayedSource" in applied[1]).toBe(false);
	});

	it("reports a displayed-source hold so the surface re-reads", async () => {
		const { writer } = recordingWriter({
			status: "no_change",
			hold: "displayed_source_unavailable",
		});
		const onDisplayedSourceHold = vi.fn();
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
			displayedSource: () => ({ lane: "preload", lease: 4 }),
			onDisplayedSourceHold,
		});
		await session
			.start({ lane: "preload", fixtureIds: [FIXTURE_1], timing: TIMING })!
			.change({ pan: positionAngleStep(1) });
		expect(onDisplayedSourceHold).toHaveBeenCalledWith("preload");
	});

	it("the Normal writer carries the source into the encoded action", async () => {
		const store = new ProgrammerValuesStore();
		store.reset(SHOW_ID, SESSION_ID, "session-a");
		store.installSnapshot(valuesSnapshot());
		const captureModeStore = new ProgrammerCaptureModeStore();
		captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
		captureModeStore.installSnapshot(captureModeSnapshot());
		const applyAction = vi.fn(
			async (_scope: { showId: string }, request: ProgrammerValuesActionRequest) =>
				({
					requestId: request.requestId,
					correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
					status: "no_change",
					revision: 1,
					captureModeRevision: 1,
					replayed: false,
					warning: null,
				}) as ProgrammerValuesActionOutcome,
		);
		const writer = new ProgrammerValuesWriter({
			scope: { showId: SHOW_ID },
			store,
			captureModeStore,
			applyAction,
			repair: async () => undefined,
			repairCaptureMode: async () => undefined,
			onError: vi.fn(),
		});
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
			displayedSource: () => ({ lane: "normal", lease: 11 }),
		});
		await session
			.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING })!
			.change({ pan: positionAngleStep(1) });
		expect(applyAction).toHaveBeenCalledTimes(1);
		expect(applyAction.mock.calls[0][1].action).toMatchObject({
			action: "apply_intent",
			displayedSource: { lane: "normal", lease: 11 },
		});
	});

	it("asks for the lease covering the gesture's displayed fixtures (a Group's members too)", () => {
		const { writer } = recordingWriter();
		const displayedSource = vi.fn((lane: "normal" | "preload", _fixtureIds: readonly string[]) => ({
			lane,
			lease: 3,
		}));
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => writer,
			createId: ids(),
			displayedSource,
		});
		session.start({ lane: "normal", fixtureIds: [FIXTURE_1], timing: TIMING });
		session.start({
			lane: "preload",
			fixtureIds: [],
			groupId: "front",
			displayedFixtureIds: [FIXTURE_1, "member-2"],
			timing: TIMING,
		});
		expect(displayedSource.mock.calls).toEqual([
			["normal", [FIXTURE_1]],
			["preload", [FIXTURE_1, "member-2"]],
		]);
	});
});

