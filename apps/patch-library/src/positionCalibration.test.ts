import { describe, expect, it } from "vitest";
import type { PatchProfileRevision } from "./contracts";
import { defaultPositionCalibration, positionCalibrationError } from "./positionCalibration";
import { newPatchFixtureCandidate, patchedFixtureCandidate, projectionToPatchedFixture } from "./state/model";
import type { FixtureDefinition } from "./wire";

const definition = {
	id: "profile", revision: 1, name: "Fixture", mode: "Standard",
	profile_id: "profile", mode_id: "mode", footprint: 1, heads: [], physical: {}, profile_snapshot: null,
} as unknown as FixtureDefinition;
const profile = {
	profileId: "profile", profileRevision: 1, contentDigest: "", manufacturer: "Test",
	name: "Fixture", fixtureType: "moving_head", patchPolicy: "dmx", referencedModes: [], profileSnapshot: null,
} as unknown as PatchProfileRevision;

describe("installed Position calibration", () => {
	it("allows unknown data, full turns and signed offsets without implying measured quality", () => {
		expect(positionCalibrationError(undefined)).toBeNull();
		expect(positionCalibrationError(null)).toBeNull();
		expect(positionCalibrationError({ ...defaultPositionCalibration(), pan_zero_degrees: -720.5, tilt_zero_degrees: 540 })).toBeNull();
	});

	it("rejects nonfinite physical values, invalid revisions and evidence without a source", () => {
		for (const degrees of [NaN, Infinity, -Infinity, 1e40]) {
			expect(positionCalibrationError({ ...defaultPositionCalibration(), pan_zero_degrees: degrees })).toMatch(/finite/);
			expect(positionCalibrationError({ ...defaultPositionCalibration(), tilt_zero_degrees: degrees })).toMatch(/finite/);
		}
		for (const revision of [-1, 0.5, 2 ** 32])
			expect(positionCalibrationError({ ...defaultPositionCalibration(), revision })).toMatch(/revision/);
		for (const quality of ["manufacturer", "measured"] as const) {
			expect(positionCalibrationError({ ...defaultPositionCalibration(), quality, source: "  " })).toMatch(/source/);
			expect(positionCalibrationError({ ...defaultPositionCalibration(), quality, source: "Rig record" })).toBeNull();
		}
		expect(positionCalibrationError({ ...defaultPositionCalibration(), source: "é".repeat(513) })).toMatch(/1024/);
	});

	it("preserves independent root and copy calibration across Architect read, unrelated edit and write", () => {
		const { fixture } = newPatchFixtureCandidate({ name: "Fixture", fixture_number: 1, definition, universe: null, address: null });
		fixture.position_calibration = { ...defaultPositionCalibration(), revision: 4, pan_zero_degrees: -720.5 };
		fixture.multipatch = [{
			id: "copy", name: "Copy", universe: null, address: null,
			split_patches: [{ split: 1, universe: null, address: null }],
			location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 }, invert_tilt: true,
			position_calibration: { ...defaultPositionCalibration(), pan_zero_degrees: 90 },
		}];
		const { input } = patchedFixtureCandidate(fixture);
		const read = projectionToPatchedFixture({ ...input, fixtureRevision: 1, logicalHeads: [] }, profile, () => definition);
		const edited = patchedFixtureCandidate({ ...read, name: "Renamed" }).input;
		expect(edited.positionCalibration).toEqual(fixture.position_calibration);
		expect(edited.multipatch[0].positionCalibration).toEqual(fixture.multipatch[0].position_calibration);
		expect(edited.multipatch[0].invertTilt).toBe(true);
		expect(edited.name).toBe("Renamed");
	});
});
