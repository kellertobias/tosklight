import { describe, it, expect } from "vitest";
import {
	colorCalibrationError,
	type InstalledColorCalibration,
} from "./colorCalibration";
const id = (n: number) =>
	`00000000-0000-0000-0000-${String(n).padStart(12, "0")}`;
export const sample = (): InstalledColorCalibration => ({
	version: 1,
	revision: 0,
	paths: [
		{
			source_identity: {
				profile_id: id(1),
				profile_revision: 2,
				profile_digest: "a".repeat(64),
				mode_id: id(2),
				head_id: id(3),
				path_id: id(4),
				model_revision: 0,
				native_layout_signature: "b".repeat(64),
			},
			emitters: [
				{
					emitter_id: id(5),
					output_gain: 0,
					provenance: { quality: "estimated", revision: 0 },
				},
			],
			measurements: [],
		},
	],
});
describe("installed Color calibration", () => {
	it("keeps absent data and zero gains distinct", () => {
		expect(colorCalibrationError(null)).toBeNull();
		expect(colorCalibrationError(sample())).toBeNull();
	});
	it("rejects malformed source and evidence without throwing", () => {
		for (const bad of [
			false,
			{},
			{ version: 1, revision: 0, paths: [null] },
			{ version: 1, revision: 0, paths: [] },
		])
			expect(colorCalibrationError(bad)).not.toBeNull();
		for (const gain of [-1, NaN, Infinity, 1e40]) {
			const value = sample();
			value.paths[0].emitters[0].output_gain = gain;
			expect(colorCalibrationError(value)).not.toBeNull();
		}
		const value = sample();
		value.paths[0].emitters[0].provenance.quality = "measured";
		expect(colorCalibrationError(value)).not.toBeNull();
		value.paths[0].emitters[0].provenance.source = "Actual meter log";
		expect(colorCalibrationError(value)).toBeNull();
	});
	it("rejects duplicate or partial identities and mixed source revisions", () => {
		const value = sample();
		value.paths.push(structuredClone(value.paths[0]));
		expect(colorCalibrationError(value)).not.toBeNull();
		value.paths[1].source_identity.head_id = id(6);
		value.paths[1].source_identity.path_id = id(7);
		value.paths[1].source_identity.profile_revision = 3;
		expect(colorCalibrationError(value)).toMatch(/one profile/);
	});
});

import {
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
	projectionToPatchedFixture,
} from "./state/model";
import type { FixtureDefinition } from "./wire";
import type { PatchProfileRevision } from "./contracts";
it("preserves independent observations through shared patch read/edit/write", () => {
	const definition = {
		schema_version: 2,
		id: "profile",
		revision: 1,
		manufacturer: "Test",
		model: "Test",
		name: "Test",
		mode: "Default",
		profile_id: "profile",
		mode_id: "mode",
		footprint: 1,
		heads: [],
		physical: {},
		profile_snapshot: null,
	} as unknown as FixtureDefinition;
	const profile = {
		profileId: "profile",
		profileRevision: 1,
		referencedModes: [],
		profileSnapshot: null,
	} as unknown as PatchProfileRevision;
	const { fixture } = newPatchFixtureCandidate({
		name: "Fixture",
		fixture_number: 1,
		definition,
		universe: null,
		address: null,
	});
	fixture.color_calibration = sample();
	fixture.multipatch = [
		{
			id: "copy",
			name: "Copy",
			universe: null,
			address: null,
			split_patches: [{ split: 1, universe: null, address: null }],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			color_calibration: { ...sample(), revision: 8 },
		},
	];
	const { input } = patchedFixtureCandidate(fixture);
	const read = projectionToPatchedFixture(
		{ ...input, fixtureRevision: 1, logicalHeads: [] },
		profile,
		() => definition,
	);
	const edited = patchedFixtureCandidate({ ...read, name: "Renamed" }).input;
	expect(edited.colorCalibration).toEqual(fixture.color_calibration);
	expect(edited.multipatch[0].colorCalibration).toEqual(
		fixture.multipatch[0].color_calibration,
	);
});
