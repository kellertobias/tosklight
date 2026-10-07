import { describe, expect, it, vi } from "vitest";
import { encodeProgrammerValuesActionRequest } from "../../api/programmerValuesWire";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { captureModeSnapshot } from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "./contracts";
import {
	FamilyGestureEditRefusedError,
	type FamilyGestureIntentInput,
	type FamilyGestureWriter,
} from "./familyGestureSession";
import {
	POSITION_TARGET_ORIGIN,
	type PositionGestureStartInput,
	PositionGestureSession,
	positionAngleStep,
	positionComponentEdits,
	positionOffsetSet,
	positionOffsetStep,
	positionTargetPoint,
} from "./positionGestureSession";
import { ProgrammerValuesStore } from "./store";
import { FIXTURE_1, SESSION_ID, SHOW_ID, valuesSnapshot } from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };
const POINT_ID = "5b0f3c1e-8a4d-4f7a-9c21-0d6e2f4a7b10";

function idSequence() {
	let next = 0;
	return () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`;
}

function harness(representation?: PositionGestureStartInput["representation"]) {
	const applies: FamilyGestureIntentInput[] = [];
	const writer: FamilyGestureWriter = {
		applyIntent: vi.fn(async (input: FamilyGestureIntentInput) => {
			applies.push(input);
			return { requestId: input.requestId };
		}),
		cancelGesture: vi.fn(() => 0),
		finishGesture: vi.fn(async () => ({ status: "no_change" })),
	};
	const onError = vi.fn();
	const session = new PositionGestureSession({
		writerFor: () => writer,
		createId: idSequence(),
		onError,
	});
	const gesture = session.start({
		lane: "normal",
		fixtureIds: [FIXTURE_1],
		timing: TIMING,
		representation,
	})!;
	return { applies, writer, onError, session, gesture };
}

describe("Position Target: atomic first offset edit", () => {
	it.each(["native", "angles"] as const)(
		"while %s, the first X edit sends Target{Origin} and target_x in ONE request",
		async (representation) => {
			const { applies, gesture, onError } = harness(representation);
			await gesture.change({
				target: POSITION_TARGET_ORIGIN,
				offset: { x: positionOffsetSet(1.5) },
			});
			expect(applies).toHaveLength(1);
			expect(applies[0]!.operation).toEqual({
				type: "component_edits",
				edits: [
					{ kind: "target", reference: { kind: "origin" } },
					{
						kind: "scalar",
						component: { kind: "target_x" },
						operation: { kind: "set", value: { kind: "value", value: 1.5 } },
					},
				],
			});
			expect(applies[0]!.attribute).toBe("position");
			expect(onError).not.toHaveBeenCalled();
		},
	);

	it("a selected stable Point reference with several offsets stays one ordered request", async () => {
		const { applies, gesture } = harness("angles");
		await gesture.change({
			target: positionTargetPoint(POINT_ID),
			offset: { z: positionOffsetStep(0.1), x: positionOffsetStep(-0.2) },
		});
		expect(applies).toHaveLength(1);
		expect(applies[0]!.operation.edits).toEqual([
			{ kind: "target", reference: { kind: "point", point_id: POINT_ID } },
			{ kind: "scalar", component: { kind: "target_x" }, operation: { kind: "relative", value: -0.2 } },
			{ kind: "scalar", component: { kind: "target_z" }, operation: { kind: "relative", value: 0.1 } },
		]);
		// Target edits are never prefixed by an Angle activation.
		expect(applies[0]!.operation.edits.some((edit) => edit.kind === "activate_angles")).toBe(false);
	});

	it("reaches the real Normal writer's wire as one apply_intent carrying both edits", async () => {
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
		const session = new PositionGestureSession({ writerFor: () => writer, createId: idSequence() });
		const gesture = session.start({
			lane: "normal",
			fixtureIds: [FIXTURE_1],
			timing: TIMING,
			representation: "angles",
		})!;
		await gesture.change({ target: POSITION_TARGET_ORIGIN, offset: { y: positionOffsetStep(0.5) } });
		gesture.end();
		await gesture.finished;
		const requests = applyAction.mock.calls.map((call) => call[1]);
		expect(requests.map((request) => request.action.action)).toEqual(["apply_intent", "finish_gesture"]);
		expect(encodeProgrammerValuesActionRequest(requests[0]!).action).toMatchObject({
			type: "apply_intent",
			attribute: "position",
			operation: {
				type: "component_edits",
				edits: [
					{ kind: "target", reference: { kind: "origin" } },
					{ kind: "scalar", component: { kind: "target_y" }, operation: { kind: "relative", value: 0.5 } },
				],
			},
		});
	});
});

describe("Position Target: offsets never invent a Target", () => {
	it.each([["native"], ["angles"], [undefined]] as const)(
		"an offset alone while %s is refused, reported once and not sent",
		async (representation) => {
			const { applies, gesture, onError, writer } = harness(representation);
			expect(gesture.change({ offset: { x: positionOffsetStep(1) } })).toBeNull();
			await Promise.resolve();
			expect(applies).toHaveLength(0);
			expect(writer.applyIntent).not.toHaveBeenCalled();
			expect(onError).toHaveBeenCalledOnce();
			expect(onError.mock.calls[0]![0]).toBeInstanceOf(FamilyGestureEditRefusedError);
			// The gesture stays open; a refusal is not an end.
			expect(gesture.isOpen).toBe(true);
		},
	);

	it("refuses an Angle and Target mix in one change", () => {
		const { gesture, onError, writer } = harness("angles");
		expect(
			gesture.change({ pan: positionAngleStep(1), target: POSITION_TARGET_ORIGIN }),
		).toBeNull();
		expect(writer.applyIntent).not.toHaveBeenCalled();
		expect(onError.mock.calls[0]![0]).toBeInstanceOf(FamilyGestureEditRefusedError);
	});

	it("the pure builder refuses too, without a session", () => {
		expect(() => positionComponentEdits({ offset: { z: positionOffsetSet(2) } }, "angles")).toThrow(
			FamilyGestureEditRefusedError,
		);
	});
});

describe("Position Target: active Target keeps its reference", () => {
	it("later offsets edit only their axis and never resend or replace the reference", async () => {
		const { applies, gesture, onError } = harness("angles");
		await gesture.change({ target: positionTargetPoint(POINT_ID), offset: { x: positionOffsetSet(1) } });
		// The caller's projection now shows Target; it says so per change.
		await gesture.change({ offset: { y: positionOffsetStep(0.25) }, representation: "target" });
		await gesture.change({ offset: { x: positionOffsetStep(-0.1) }, representation: "target" });
		expect(applies.map((input) => input.operation.edits)).toEqual([
			[
				{ kind: "target", reference: { kind: "point", point_id: POINT_ID } },
				{ kind: "scalar", component: { kind: "target_x" }, operation: { kind: "set", value: { kind: "value", value: 1 } } },
			],
			[{ kind: "scalar", component: { kind: "target_y" }, operation: { kind: "relative", value: 0.25 } }],
			[{ kind: "scalar", component: { kind: "target_x" }, operation: { kind: "relative", value: -0.1 } }],
		]);
		expect(new Set(applies.map((input) => input.undoGroup)).size).toBe(1);
		expect(onError).not.toHaveBeenCalled();
	});

	it("a gesture started with Target active edits offsets without a reference", async () => {
		const { applies, gesture } = harness("target");
		await gesture.change({ offset: { z: positionOffsetStep(0.01) } });
		expect(applies[0]!.operation.edits).toEqual([
			{ kind: "scalar", component: { kind: "target_z" }, operation: { kind: "relative", value: 0.01 } },
		]);
	});

	it("a change's representation overrides the start default", () => {
		const { gesture, writer } = harness("target");
		expect(gesture.change({ offset: { x: positionOffsetStep(1) }, representation: "angles" })).toBeNull();
		expect(writer.applyIntent).not.toHaveBeenCalled();
	});

	it("choosing another Point while Target is active sends only the reference", async () => {
		const { applies, gesture } = harness("target");
		await gesture.change({ target: POSITION_TARGET_ORIGIN });
		expect(applies[0]!.operation.edits).toEqual([{ kind: "target", reference: { kind: "origin" } }]);
	});

	it("Pan/Tilt while Target is stated active prefixes one explicit Angle activation", async () => {
		const { applies, gesture } = harness("target");
		await gesture.change({ pan: positionAngleStep(3), activateAngles: true });
		await gesture.change({ tilt: positionAngleStep(2) });
		expect(applies.map((input) => input.operation.edits.map((edit) => edit.kind))).toEqual([
			["activate_angles", "scalar"],
			["activate_angles", "scalar"],
		]);
	});

	it("a nil Point UUID is rejected by the generated-wire validator and not sent", () => {
		const { gesture, onError, writer } = harness("angles");
		expect(
			gesture.change({
				target: positionTargetPoint("00000000-0000-0000-0000-000000000000"),
				offset: { x: positionOffsetSet(0) },
			}),
		).toBeNull();
		expect(writer.applyIntent).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledOnce();
	});
});
