import { afterEach, describe, expect, it } from "vitest";
import { colorAdoptionNotice } from "../../../../features/familyEncoders/colorAdoptionNotice";
import { FamilyEncoderBinding, type FamilyEncoderTarget } from "./familyEncoderBinding";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
	manualTimers,
	RED,
	sequentialIds,
} from "./familyEncoderTestSupport";
import { nativeColorSlot } from "./nativeColorSlots";
import { HEAD, nativeControl, wheelControl } from "./nativeColorTestSupport";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };
const target: FamilyEncoderTarget = { lane: "normal", groupId: null, timing: TIMING };
const GREEN = nativeControl(1);
const SLOT = nativeColorSlot(GREEN, GREEN.functions[0], [FIXTURE_A, FIXTURE_B]);
const REFERENCE = { fixture_id: FIXTURE_A, head_id: HEAD };

function rig(reference: typeof REFERENCE | null = REFERENCE) {
	const normal = fakeWriter();
	const clock = manualTimers();
	const binding = new FamilyEncoderBinding({
		writerFor: () => normal,
		createId: sequentialIds(),
		timers: clock.timers,
		nativeReference: () => reference,
		semanticColorAdoption: () => colorAdoptionNotice.semanticInput(),
	});
	return { normal, clock, binding };
}

const sent = (writer: ReturnType<typeof fakeWriter>) =>
	writer.applyIntent.mock.calls.map(([input]) => input as Record<string, unknown>);

afterEach(() => colorAdoptionNotice.reset());

describe("family encoder binding: Direct (native) slots (TL-554)", () => {
	it("sends full-width native edits that name the reference head, one Undo group per turn", () => {
		const { normal, clock, binding } = rig();
		binding.detent(SLOT, "up", target);
		binding.detent(SLOT, "right", target);
		binding.step(SLOT, -1, target);
		const edits = sent(normal);
		expect(edits).toHaveLength(3);
		expect(edits[0]).toMatchObject({
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			attribute: "color",
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: {
							channel_id: GREEN.channel_id,
							function_id: GREEN.functions[0].function_id,
						},
						operation: { kind: "relative", value: 257 },
					},
				],
			},
		});
		expect(
			edits.map(
				(edit) =>
					(edit.operation as { edits: { operation: unknown }[] }).edits[0].operation,
			),
		).toEqual([
			{ kind: "relative", value: 257 },
			{ kind: "relative", value: 2570 },
			{ kind: "relative", value: -1 },
		]);
		expect(new Set(edits.map((edit) => edit.undoGroup)).size).toBe(1);
		clock.fireAll();
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});

	it("gives hardware detents and software steps identical native edits", () => {
		const hardware = rig();
		const software = rig();
		hardware.binding.detent(SLOT, "down", target);
		software.binding.step(SLOT, -SLOT.descriptor.step, target);
		const strip = (writer: ReturnType<typeof fakeWriter>) =>
			sent(writer).map(({ operation, colorAdoption, attribute }) => ({
				operation,
				colorAdoption,
				attribute,
			}));
		expect(strip(hardware.normal)).toEqual(strip(software.normal));
	});

	it("sets an exact 32-bit value as one complete gesture", () => {
		const { normal, binding } = rig();
		const white = nativeControl(3);
		const slot = nativeColorSlot(white, white.functions[0], [FIXTURE_A]);
		binding.set(slot, 4_294_967_295, target);
		expect(
			(sent(normal)[0].operation as { edits: { operation: unknown }[] }).edits[0].operation,
		).toEqual({ kind: "set", value: 4_294_967_295 });
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});

	it("sends nothing without a verified reference", () => {
		const unreferenced = rig(null);
		unreferenced.binding.step(SLOT, 1, target);
		expect(unreferenced.normal.applyIntent).not.toHaveBeenCalled();
	});

	it("steps a discrete wheel one choice per detent or step on every surface (TL-544 G4)", () => {
		const wheel = wheelControl(4);
		const red = nativeColorSlot(wheel, wheel.functions[1], [FIXTURE_A]);
		const hardware = rig();
		const software = rig();
		// A detent of either size and a software step each move exactly one choice.
		hardware.binding.detent(red, "right", target);
		software.binding.step(red, red.descriptor.step, target);
		const operations = (writer: ReturnType<typeof fakeWriter>) =>
			sent(writer).map(
				(edit) => (edit.operation as { edits: unknown[] }).edits[0],
			);
		const blue = {
			kind: "native",
			binding: { channel_id: wheel.channel_id, function_id: wheel.functions[2].function_id },
			operation: { kind: "set", value: 20 },
		};
		expect(operations(hardware.normal)).toEqual([blue]);
		expect(operations(software.normal)).toEqual([blue]);
		// The slot still shows Red: the next detent continues from Blue (wrapping to Open).
		hardware.binding.detent(red, "up", target);
		expect(operations(hardware.normal)[1]).toEqual({
			...blue,
			binding: { ...blue.binding, function_id: wheel.functions[0].function_id },
			operation: { kind: "set", value: 0 },
		});
		expect(new Set(sent(hardware.normal).map((edit) => edit.undoGroup)).size).toBe(1);
		// A chosen value (value pad, Direct section choice) names its own function.
		const { normal, binding } = rig();
		binding.set(red, 3, target);
		expect(operations(normal)).toEqual([
			{
				...blue,
				binding: { ...blue.binding, function_id: wheel.functions[0].function_id },
				operation: { kind: "set", value: 3 },
			},
		]);
	});

	it("never shares a gesture between Semantic and Direct edits; an explicit start rides along", () => {
		const { normal, binding } = rig();
		binding.step(RED, 0.01, target);
		binding.step(SLOT, 1, target);
		const [semantic, direct] = sent(normal);
		expect(semantic.undoGroup).not.toBe(direct.undoGroup);
		expect(semantic.colorAdoption).toBeUndefined();
		colorAdoptionNotice.choose([0, 0, 0]);
		binding.step(RED, 0.01, target);
		expect(sent(normal)[2]).toMatchObject({
			colorAdoption: { explicitStart: { rgb: [0, 0, 0] } },
		});
	});
});
