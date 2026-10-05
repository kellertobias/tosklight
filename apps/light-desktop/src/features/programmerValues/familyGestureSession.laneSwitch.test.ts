import { describe, expect, it, vi } from "vitest";
import { POSITION_GESTURE_FAMILY, positionAngleStep } from "./positionGestureSession";
import {
	type FamilyGestureFinishInput,
	type FamilyGestureIntentInput,
	type FamilyGestureLane,
	FamilyGestureSession,
	type FamilyGestureWriter,
} from "./familyGestureSession";
import { FIXTURE_1 } from "./testFixtures";

const FADE = { fade: true, fadeMillis: 2_000, delayMillis: null };
const IMMEDIATE = { fade: false, fadeMillis: null, delayMillis: null };

type Entry =
	| { lane: FamilyGestureLane; step: "apply"; input: FamilyGestureIntentInput }
	| { lane: FamilyGestureLane; step: "finish"; input: FamilyGestureFinishInput }
	| { lane: FamilyGestureLane; step: "cancel" }
	| { lane: "producer"; step: "stop" };

function writer(lane: FamilyGestureLane, log: Entry[]): FamilyGestureWriter {
	return {
		applyIntent: vi.fn(async (input: FamilyGestureIntentInput) => {
			log.push({ lane, step: "apply", input });
			return {};
		}),
		cancelGesture: vi.fn(() => {
			log.push({ lane, step: "cancel" });
			return 0;
		}),
		finishGesture: vi.fn(async (input: FamilyGestureFinishInput) => {
			log.push({ lane, step: "finish", input });
			return {};
		}),
	};
}

describe("a gesture that outlives Preload capture (2026-10-05)", () => {
	it("finishes its Preload part and continues the motion on the Normal Programmer", async () => {
		const log: Entry[] = [];
		const writers = { normal: writer("normal", log), preload: writer("preload", log) };
		let lane: FamilyGestureLane = "preload";
		let next = 0;
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: (name) => writers[name],
			currentLane: () => lane,
			laneTiming: (name) => (name === "normal" ? IMMEDIATE : null),
			createId: () => `id-${++next}`,
		});
		const handle = session.start({
			lane: "preload",
			fixtureIds: [FIXTURE_1],
			timing: FADE,
			stopProducer: () => log.push({ lane: "producer", step: "stop" }),
		})!;
		await handle.change({ pan: positionAngleStep(5) });

		// Preload capture ends while the joystick is still held.
		lane = "normal";
		await handle.change({ pan: positionAngleStep(5) });
		await handle.change({ pan: positionAngleStep(5) });
		expect(handle.isOpen).toBe(true);
		expect(handle.lane).toBe("normal");
		handle.end();
		await Promise.resolve();

		const applies = log.filter((entry) => entry.step === "apply") as Extract<Entry, { step: "apply" }>[];
		expect(applies.map((entry) => entry.lane)).toEqual(["preload", "normal", "normal"]);
		const [preloadEdit, ...normalEdits] = applies;
		// The Normal continuation is its own Undo step without the Programmer Fade.
		expect(new Set(normalEdits.map((entry) => entry.input.undoGroup)).size).toBe(1);
		expect(normalEdits[0].input.undoGroup).not.toBe(preloadEdit.input.undoGroup);
		expect(normalEdits[0].input.timing).toEqual(IMMEDIATE);
		expect(preloadEdit.input.timing).toEqual(FADE);

		const finishes = log.filter((entry) => entry.step === "finish") as Extract<Entry, { step: "finish" }>[];
		expect(finishes.map((entry) => entry.lane)).toEqual(["preload", "normal"]);
		// The Preload part keeps everything it already sent.
		expect(finishes[0].input).toMatchObject({ undoGroup: preloadEdit.input.undoGroup, keepAdmittedEdits: true });
		expect(finishes[1].input.undoGroup).toBe(normalEdits[0].input.undoGroup);
		// The producer was stopped exactly once, by the operator's release.
		expect(log.filter((entry) => entry.lane === "producer")).toHaveLength(1);
		expect(session.active).toBeNull();
	});

	it("keeps its start lane when no current lane is reported", async () => {
		const log: Entry[] = [];
		const writers = { normal: writer("normal", log), preload: writer("preload", log) };
		const session = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: (name) => writers[name],
		});
		const handle = session.start({ lane: "preload", fixtureIds: [FIXTURE_1], timing: FADE })!;
		await handle.change({ pan: positionAngleStep(5) });
		await handle.change({ pan: positionAngleStep(5) });
		expect(log.filter((entry) => entry.step === "apply").map((entry) => entry.lane)).toEqual([
			"preload",
			"preload",
		]);
	});
});
