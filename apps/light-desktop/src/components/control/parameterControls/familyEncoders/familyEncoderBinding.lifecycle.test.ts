import { describe, expect, it } from "vitest";
import { POSITION_TARGET_ORIGIN, positionTargetPoint } from "../../../../features/programmerValues/positionGestureSession";
import { FamilyEncoderBinding, type FamilyEncoderTarget } from "./familyEncoderBinding";
import {
	FIXTURE_A,
	FIXTURE_B,
	FOCUS,
	fakeWriter,
	manualTimers,
	PAN,
	POINT,
	sequentialIds,
} from "./familyEncoderTestSupport";
import { nativeColorSlot } from "./nativeColorSlots";
import { HEAD, nativeControl, wheelControl } from "./nativeColorTestSupport";

/**
 * TL-544 G12 (encoder gestures end on release, blur and hidden), G4 (the Point slot on software
 * steps) and G6 (Direct `[THRU]` spreads) on the family encoder binding.
 */

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };
const target: FamilyEncoderTarget = { lane: "normal", groupId: null, timing: TIMING };
const POINT_ONE = positionTargetPoint("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
const POINT_TWO = positionTargetPoint("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb");
const CHOICES = [POSITION_TARGET_ORIGIN, POINT_ONE, POINT_TWO];

function rig() {
	const normal = fakeWriter();
	const clock = manualTimers();
	const binding = new FamilyEncoderBinding({
		writerFor: () => normal,
		createId: sequentialIds(),
		timers: clock.timers,
		pointChoices: () => CHOICES,
		nativeReference: () => ({ fixture_id: FIXTURE_A, head_id: HEAD }),
	});
	return { normal, clock, binding };
}

const sent = (writer: ReturnType<typeof fakeWriter>) =>
	writer.applyIntent.mock.calls.map(([input]) => input as Record<string, unknown>);
const finishes = (writer: ReturnType<typeof fakeWriter>) =>
	writer.finishGesture.mock.calls.map(([input]) => input as Record<string, unknown>);

function windowTarget() {
	const window = new EventTarget() as Window;
	const document = Object.assign(new EventTarget(), { hidden: false }) as Document & {
		hidden: boolean;
	};
	return { window, document };
}

describe("family encoder gestures end on release, blur and hidden (TL-544 G12)", () => {
	it("finishes an open turn once on explicit release, keeping its steps in one Undo group", () => {
		const { normal, clock, binding } = rig();
		binding.step(PAN, 1, target);
		binding.step(PAN, 1, target);
		expect(binding.finishGestures()).toBe(true);
		expect(normal.cancelGesture).not.toHaveBeenCalled();
		expect(finishes(normal)).toHaveLength(1);
		expect(finishes(normal)[0]).toMatchObject({
			attribute: "position",
			undoGroup: sent(normal)[0].undoGroup,
			keepAdmittedEdits: true,
		});
		expect(new Set(sent(normal).map((edit) => edit.undoGroup)).size).toBe(1);
		// The idle end no longer fires a second Finish, and nothing is left to finish.
		clock.fireAll();
		expect(binding.finishGestures()).toBe(false);
		expect(normal.finishGesture).toHaveBeenCalledOnce();
		// The next turn is a new gesture.
		binding.step(PAN, 1, target);
		expect(sent(normal)[2].undoGroup).not.toBe(sent(normal)[0].undoGroup);
	});

	it("finishes every owner's open gesture on window blur, once", () => {
		const { normal, binding } = rig();
		const guarded = windowTarget();
		const detach = binding.attachWindowGuards(guarded);
		binding.detent(PAN, "up", target);
		binding.detent(FOCUS, "up", target);
		guarded.window.dispatchEvent(new Event("blur"));
		guarded.window.dispatchEvent(new Event("blur"));
		expect(finishes(normal).map((finish) => finish.attribute).sort()).toEqual([
			"focus",
			"position",
		]);
		detach();
	});

	it("finishes on the document becoming hidden, not on becoming visible, and detaches", () => {
		const { normal, binding } = rig();
		const guarded = windowTarget();
		const detach = binding.attachWindowGuards(guarded);
		binding.detent(PAN, "up", target);
		guarded.document.dispatchEvent(new Event("visibilitychange"));
		expect(normal.finishGesture).not.toHaveBeenCalled();
		guarded.document.hidden = true;
		guarded.document.dispatchEvent(new Event("visibilitychange"));
		expect(normal.finishGesture).toHaveBeenCalledOnce();
		detach();
		binding.detent(PAN, "up", target);
		guarded.window.dispatchEvent(new Event("blur"));
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});
});

describe("Point slot on software steps (TL-544 G4)", () => {
	const references = (writer: ReturnType<typeof fakeWriter>) =>
		sent(writer).map(
			(input) =>
				(input.operation as { edits: { reference?: unknown }[] }).edits[0].reference,
		);

	it("cycles the ordered Point choices exactly like detents, one Target gesture per step", () => {
		const software = rig();
		const hardware = rig();
		const at = (reference: (typeof CHOICES)[number]): FamilyEncoderTarget => ({
			...target,
			positionRepresentation: "target",
			positionTargetReference: reference,
		});
		software.binding.step(POINT, 1, at(POSITION_TARGET_ORIGIN));
		software.binding.step(POINT, 10, at(POINT_ONE));
		software.binding.step(POINT, 1, at(POINT_TWO));
		software.binding.step(POINT, -1, at(POSITION_TARGET_ORIGIN));
		hardware.binding.detent(POINT, "up", at(POSITION_TARGET_ORIGIN));
		hardware.binding.detent(POINT, "right", at(POINT_ONE));
		hardware.binding.detent(POINT, "up", at(POINT_TWO));
		hardware.binding.detent(POINT, "down", at(POSITION_TARGET_ORIGIN));
		expect(references(software.normal)).toEqual([POINT_ONE, POINT_TWO, POSITION_TARGET_ORIGIN, POINT_TWO]);
		expect(references(hardware.normal)).toEqual(references(software.normal));
		expect(software.normal.finishGesture).toHaveBeenCalledTimes(4);
		expect(new Set(sent(software.normal).map((edit) => edit.undoGroup)).size).toBe(4);
	});

	it("starts at the first choice going up and the last going down without a shared reference", () => {
		const { normal, binding } = rig();
		binding.step(POINT, 1, target);
		binding.step(POINT, -1, target);
		expect(references(normal)).toEqual([POSITION_TARGET_ORIGIN, POINT_TWO]);
		expect(binding.step(POINT, 0, target)).toBeNull();
	});
});

describe("Direct (native) [THRU] spreads (TL-544 G6)", () => {
	const GREEN = nativeControl(0);
	const SLOT = nativeColorSlot(GREEN, GREEN.functions[0], [FIXTURE_A, FIXTURE_B]);

	it("publishes spread on a continuous function and never on a discrete one", () => {
		expect(SLOT.descriptor.spread).toBe(true);
		const wheel = wheelControl(4);
		expect(nativeColorSlot(wheel, wheel.functions[1], []).descriptor.spread).toBe(false);
	});

	it("sends one complete native spread of rounded integers inside the function", () => {
		const { normal, binding } = rig();
		binding.spread(SLOT, [12.4, 300, -5], target);
		expect(sent(normal)).toHaveLength(1);
		expect(sent(normal)[0]).toMatchObject({
			attribute: "color",
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: { channel_id: GREEN.channel_id, function_id: GREEN.functions[0].function_id },
						operation: { kind: "spread", value: [12, 255, 0] },
					},
				],
			},
		});
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});

	it("refuses a spread of fewer than two points or on a discrete function", () => {
		const { normal, binding } = rig();
		expect(binding.spread(SLOT, [10], target)).toBeNull();
		const wheel = wheelControl(4);
		expect(binding.spread(nativeColorSlot(wheel, wheel.functions[1], []), [10, 12], target)).toBeNull();
		expect(normal.applyIntent).not.toHaveBeenCalled();
	});
});
