import { describe, expect, it } from "vitest";
import {
	FAMILY_ENCODER_IDLE_END_MILLIS,
	FamilyEncoderBinding,
	type FamilyEncoderTarget,
	familyDetentDelta,
	familySlotSpreads,
} from "./familyEncoderBinding";
import {
	FIXTURE_A,
	FIXTURE_B,
	FOCUS,
	fakeWriter,
	manualTimers,
	PAN,
	POINT,
	RED,
	sequentialIds,
	TARGET_X,
	TILT,
	WHEEL,
	ZOOM,
	ZOOM_UNKNOWN_CONVENTION,
} from "./familyEncoderTestSupport";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };

function rig() {
	const normal = fakeWriter();
	const preload = fakeWriter();
	const clock = manualTimers();
	const binding = new FamilyEncoderBinding({
		writerFor: (lane) => (lane === "preload" ? preload : normal),
		createId: sequentialIds(),
		timers: clock.timers,
	});
	return { normal, preload, clock, binding };
}

const normalTarget: FamilyEncoderTarget = {
	lane: "normal",
	groupId: null,
	timing: TIMING,
};
const preloadTarget: FamilyEncoderTarget = { ...normalTarget, lane: "preload" };

function sentOperations(writer: ReturnType<typeof fakeWriter>) {
	return writer.applyIntent.mock.calls.map(
		([input]) => (input as { operation: unknown }).operation,
	);
}

describe("family encoder binding", () => {
	it("routes a Normal relative step to the slot's component edit, never a normalized value", () => {
		const { normal, preload, binding } = rig();

		binding.step(PAN, 2, normalTarget);

		expect(preload.applyIntent).not.toHaveBeenCalled();
		expect(normal.applyIntent).toHaveBeenCalledOnce();
		expect(normal.applyIntent.mock.calls[0]?.[0]).toMatchObject({
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			groupId: null,
			attribute: "position",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "pan" },
						operation: { kind: "relative", value: 2 },
					},
				],
			},
		});
		for (const operation of sentOperations(normal))
			expect(operation).not.toMatchObject({ type: "relative_step" });
	});

	it("routes a Preload relative step to the Preload writer with that owner's attribute", () => {
		const { normal, preload, binding } = rig();

		binding.step(FOCUS, -0.01, preloadTarget);
		binding.step(RED, 0.1, preloadTarget);

		expect(normal.applyIntent).not.toHaveBeenCalled();
		const calls = preload.applyIntent.mock.calls.map(([input]) => input);
		expect(calls).toMatchObject([
			{
				attribute: "focus",
				operation: {
					type: "component_edits",
					edits: [
						{
							kind: "scalar",
							component: { kind: "focus" },
							operation: { kind: "relative", value: -0.01 },
						},
					],
				},
			},
			{
				attribute: "color",
				operation: {
					type: "component_edits",
					edits: [
						{
							kind: "scalar",
							component: { kind: "color", component: "red" },
							operation: { kind: "relative", value: 0.1 },
						},
					],
				},
			},
		]);
	});

	it("keeps consecutive detents in one Undo group and finishes exactly once after idle", () => {
		const { normal, clock, binding } = rig();

		binding.detent(PAN, "up", normalTarget);
		binding.detent(TILT, "down", normalTarget);
		binding.detent(ZOOM, "right", normalTarget);

		const [pan, tilt, zoom] = normal.applyIntent.mock.calls.map(
			([input]) => input as { undoGroup: string; attribute: string },
		);
		expect(pan?.undoGroup).toBe(tilt?.undoGroup);
		expect(zoom?.undoGroup).not.toBe(pan?.undoGroup);
		expect(normal.finishGesture).not.toHaveBeenCalled();

		clock.fireAll();

		// Detents are discrete steps: the idle end keeps every admitted detent ahead of its Finish.
		expect(normal.finishGesture.mock.calls.map(([input]) => input)).toEqual([
			{ requestId: expect.any(String), attribute: "position", undoGroup: pan?.undoGroup, keepAdmittedEdits: true },
			{ requestId: expect.any(String), attribute: "zoom", undoGroup: zoom?.undoGroup, keepAdmittedEdits: true },
		]);
		expect(normal.cancelGesture).not.toHaveBeenCalled();
		expect(FAMILY_ENCODER_IDLE_END_MILLIS).toBe(250);
	});

	it("sends the same component edits for a hardware detent and a software step (parity)", () => {
		const hardware = rig();
		const software = rig();
		for (const [value, delta] of [
			["up", PAN.descriptor.step],
			["down", -PAN.descriptor.step],
			["right", PAN.descriptor.step * 10],
			["left", -PAN.descriptor.step * 10],
		] as const) {
			expect(familyDetentDelta(PAN, value)).toBe(delta);
			hardware.binding.detent(PAN, value, normalTarget);
			software.binding.step(PAN, delta, normalTarget);
		}
		expect(sentOperations(hardware.normal)).toEqual(sentOperations(software.normal));
		expect(sentOperations(hardware.normal)).toHaveLength(4);
	});

	it("activates Target with the first X/Y/Z edit and edits only the offset once Target is active", () => {
		const { normal, clock, binding } = rig();

		binding.step(TARGET_X, 0.1, normalTarget);
		clock.fireAll();
		binding.step(TARGET_X, 0.1, {
			...normalTarget,
			positionRepresentation: "target",
		});

		expect(sentOperations(normal)).toEqual([
			{
				type: "component_edits",
				edits: [
					{ kind: "target", reference: { kind: "origin" } },
					{
						kind: "scalar",
						component: { kind: "target_x" },
						operation: { kind: "relative", value: 0.1 },
					},
				],
			},
			{
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "target_x" },
						operation: { kind: "relative", value: 0.1 },
					},
				],
			},
		]);
	});

	it("sends a typed value as one complete gesture and ignores display-only slots", () => {
		const { normal, binding } = rig();

		binding.set(ZOOM, 30, normalTarget);
		binding.step(WHEEL, 1, normalTarget);
		expect(binding.detent(WHEEL, "up", normalTarget)).toBe(true);

		expect(sentOperations(normal)).toEqual([
			{
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "zoom" },
						operation: { kind: "set", value: { kind: "value", value: 30 } },
					},
				],
			},
		]);
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});

	it("sends no Zoom edit while the selection publishes no convention, but consumes the detent", () => {
		const { normal, preload, clock, binding } = rig();

		expect(binding.detent(ZOOM_UNKNOWN_CONVENTION, "up", normalTarget)).toBe(true);
		expect(binding.detent(ZOOM_UNKNOWN_CONVENTION, "left", preloadTarget)).toBe(true);
		expect(binding.step(ZOOM_UNKNOWN_CONVENTION, 1, normalTarget)).toBeNull();
		expect(binding.set(ZOOM_UNKNOWN_CONVENTION, 30, normalTarget)).toBeNull();
		clock.fireAll();

		for (const writer of [normal, preload]) {
			expect(writer.applyIntent).not.toHaveBeenCalled();
			expect(writer.finishGesture).not.toHaveBeenCalled();
			expect(writer.cancelGesture).not.toHaveBeenCalled();
		}
		// Focus on the same selection is unaffected.
		binding.step(FOCUS, 0.01, normalTarget);
		expect(normal.applyIntent).toHaveBeenCalledOnce();
	});

	it("turns the Point slot through its reference choices as Target edits", () => {
		const { normal, binding } = rig();

		binding.detent(POINT, "up", normalTarget);
		binding.detent(POINT, "up", {
			...normalTarget,
			positionRepresentation: "target",
			positionTargetReference: { kind: "origin" },
		});

		expect(sentOperations(normal)).toEqual([
			{
				type: "component_edits",
				edits: [{ kind: "target", reference: { kind: "origin" } }],
			},
		]);
		expect(normal.finishGesture).toHaveBeenCalledOnce();
	});

	it("targets a selected group instead of fixture ids", () => {
		const { normal, binding } = rig();
		binding.step(FOCUS, 0.01, { ...normalTarget, groupId: "group-1" });
		expect(normal.applyIntent.mock.calls[0]?.[0]).toMatchObject({
			fixtureIds: [],
			groupId: "group-1",
		});
	});

	it("sends no Position edit, Target choice or Finish for a selection without Position physical data (TL-637)", () => {
		const { normal, binding, clock } = rig();
		const unsupported: FamilyEncoderTarget = { ...normalTarget, positionUnsupported: true };
		expect(binding.detent(PAN, "up", unsupported)).toBe(true);
		expect(binding.detent(TILT, "right", unsupported)).toBe(true);
		expect(binding.step(PAN, 1, unsupported)).toBeNull();
		expect(binding.set(TILT, 10, unsupported)).toBeNull();
		expect(binding.step(TARGET_X, 0.1, unsupported)).toBeNull();
		expect(binding.chooseTarget(POINT, { kind: "origin" }, unsupported)).toBeNull();
		expect(binding.detent(POINT, "up", unsupported)).toBe(true);
		expect(clock.pending).toBe(0);
		clock.fireAll();
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
		// The flag is Position-only: Focus on the same target still edits.
		binding.step(FOCUS, 0.01, unsupported);
		expect(sentOperations(normal)).toHaveLength(1);
	});
	it("sends a typed THRU spread as one ordered component spread over the selection (PROG-002)", () => {
		const { normal, preload, clock, binding } = rig();

		binding.spread(PAN, [270, -270, 270], normalTarget);
		binding.spread(RED, [0.8, 0.2], preloadTarget);
		binding.spread(ZOOM, [40, 10], { ...normalTarget, groupId: "group-1" });

		expect(normal.applyIntent.mock.calls.map(([input]) => input)).toMatchObject([
			{
				fixtureIds: [FIXTURE_A, FIXTURE_B],
				groupId: null,
				attribute: "position",
				operation: {
					type: "component_edits",
					edits: [
						{
							kind: "scalar",
							component: { kind: "pan" },
							// The typed order is kept: never sorted or collapsed.
							operation: { kind: "set", value: { kind: "spread", value: [270, -270, 270] } },
						},
					],
				},
			},
			{
				// A Group is addressed as that Group: the backend spreads over its ordered members.
				fixtureIds: [],
				groupId: "group-1",
				attribute: "zoom",
				operation: {
					type: "component_edits",
					edits: [
						{
							kind: "scalar",
							component: { kind: "zoom" },
							operation: { kind: "set", value: { kind: "spread", value: [40, 10] } },
						},
					],
				},
			},
		]);
		expect(sentOperations(preload)).toEqual([
			{
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "color", component: "red" },
						operation: { kind: "set", value: { kind: "spread", value: [0.8, 0.2] } },
					},
				],
			},
		]);
		// Each spread is a complete request: one Finish each, keeping its admitted edit.
		expect(normal.finishGesture).toHaveBeenCalledTimes(2);
		expect(preload.finishGesture).toHaveBeenCalledOnce();
		expect(normal.cancelGesture).not.toHaveBeenCalled();
		expect(clock.pending).toBe(0);
	});

	it("refuses a spread on a slot without published spread, with one point, a non-finite point or unsupported Position", () => {
		const { normal, binding } = rig();

		expect(familySlotSpreads(PAN)).toBe(true);
		expect(familySlotSpreads(WHEEL)).toBe(false);
		expect(familySlotSpreads(POINT)).toBe(false);
		expect(binding.spread(WHEEL, [1, 3], normalTarget)).toBeNull();
		expect(binding.spread(POINT, [0, 1], normalTarget)).toBeNull();
		expect(binding.spread(PAN, [10], normalTarget)).toBeNull();
		expect(binding.spread(PAN, [10, Number.NaN], normalTarget)).toBeNull();
		expect(binding.spread(TILT, [10, 20], { ...normalTarget, positionUnsupported: true })).toBeNull();
		expect(binding.spread(ZOOM_UNKNOWN_CONVENTION, [10, 20], normalTarget)).toBeNull();
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
	});
});
