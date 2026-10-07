import { describe, expect, it } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../api/familyEncoderModels";
import {
	familySlotDisplay,
	positionSelectionState,
	positionSlotUnsupported,
	type ProgrammerValueEntry,
} from "./familyEncoderDisplay";
import { FIXTURE_A, FIXTURE_B, FOCUS, PAN, POINT, TARGET_X, ZOOM } from "./familyEncoderTestSupport";

const angles = (fixtureId: string, pan: number): ProgrammerValueEntry => ({
	fixtureId,
	attribute: "position",
	value: {
		kind: "position",
		value: {
			kind: "angles",
			pan_degrees: { kind: "value", value: pan },
			tilt_degrees: { kind: "value", value: 0 },
		},
	},
});

function readouts(...pans: Array<number | null>): OutputReadoutSnapshot {
	return {
		lane: "normal",
		owners: pans.map((pan, index) => ({
			fixture_id: [FIXTURE_A, FIXTURE_B][index],
			position: {
				available: true,
				commands: [],
				common: pan === null ? null : { pan_degrees: pan, tilt_degrees: 0 },
			},
		})),
	} as unknown as OutputReadoutSnapshot;
}

describe("family encoder display", () => {
	it("reads Pan from the displayed source's resolved angles before the request", () => {
		const values = [angles(FIXTURE_A, 10), angles(FIXTURE_B, 10)];
		expect(familySlotDisplay(PAN, { programmerValues: values, readouts: readouts(42.5, 42.5) })).toEqual({
			value: 42.5,
			text: "42.5°",
			source: "resolved",
		});
		expect(familySlotDisplay(PAN, { programmerValues: values, readouts: readouts(42.5, null) })).toMatchObject({
			value: null,
			text: "Mixed",
			source: "resolved",
		});
		expect(familySlotDisplay(PAN, { programmerValues: values, readouts: null })).toEqual({
			value: 10,
			text: "10°",
			source: "requested",
		});
	});

	it("reads Focus in percent from the requested value and nothing for an absent value", () => {
		const focus: ProgrammerValueEntry[] = [
			{ fixtureId: FIXTURE_A, attribute: "focus", value: { kind: "normalized", value: 0.25 } },
			{ fixtureId: FIXTURE_B, attribute: "focus", value: { kind: "normalized", value: 0.25 } },
		];
		expect(familySlotDisplay(FOCUS, { programmerValues: focus, readouts: null }).text).toBe("25%");
		expect(familySlotDisplay(TARGET_X, { programmerValues: [], readouts: null })).toEqual({
			value: null,
			text: "—",
			source: "none",
		});
	});

	it("states a representation only when the whole selection shares one", () => {
		expect(positionSelectionState([angles(FIXTURE_A, 1)], [FIXTURE_A, FIXTURE_B]).representation).toBeUndefined();
		expect(
			positionSelectionState([angles(FIXTURE_A, 1), angles(FIXTURE_B, 2)], [FIXTURE_A, FIXTURE_B]).representation,
		).toBe("angles");
	});

	it("calls Position unsupported only on a valid source that reports no pose and nothing requested (TL-637)", () => {
		const unposed = (overrides: Partial<OutputReadoutSnapshot> = {}, available = [false, false]) =>
			({
				lane: "normal",
				lease: 5,
				revision: 1,
				owners: available.map((value, index) => ({
					fixture_id: [FIXTURE_A, FIXTURE_B][index],
					position: { available: value, commands: [], common: null },
				})),
				...overrides,
			}) as unknown as OutputReadoutSnapshot;
		const none = { programmerValues: [] as ProgrammerValueEntry[], readouts: unposed() };
		for (const slot of [PAN, POINT, TARGET_X]) expect(positionSlotUnsupported(slot, none)).toBe(true);
		expect(familySlotDisplay(PAN, none)).toEqual({ value: null, text: "—", source: "none", unsupported: true });
		// Not Position, not a valid source, a partly posed selection or a requested value: editable.
		expect(positionSlotUnsupported(ZOOM, none)).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, readouts: null })).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, readouts: unposed({ lease: null }) })).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, readouts: unposed({ unavailable: "no_accepted_frame" }) })).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, readouts: unposed({}, [true, false]) })).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, readouts: unposed({}, [false]) })).toBe(false);
		expect(positionSlotUnsupported(PAN, { ...none, programmerValues: [angles(FIXTURE_B, 0)] })).toBe(false);
		expect(familySlotDisplay(PAN, { ...none, programmerValues: [angles(FIXTURE_A, 3), angles(FIXTURE_B, 3)] })).toMatchObject({ value: 3, source: "requested" });
	});
});
