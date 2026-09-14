import { describe, expect, it } from "vitest";
import type { PatchProfileRevision } from "../contracts";
import type { FixtureDefinition, PatchedFixture } from "../wire";
import { patchedFixtureCandidate, projectionToPatchedFixture } from "./model";

const definition = {
	id: "profile-hall",
	revision: 1,
	name: "Hall",
	mode: "Model",
	profile_id: "profile-hall",
	mode_id: "mode-model",
	footprint: 0,
	heads: [],
	physical: {},
	profile_snapshot: null,
} as unknown as FixtureDefinition;

const profile = {
	profileId: "profile-hall",
	profileRevision: 1,
	contentDigest: "",
	manufacturer: "Imported models",
	name: "Hall",
	fixtureType: "venue",
	patchPolicy: "visual_only",
	referencedModes: [],
	profileSnapshot: null,
} as unknown as PatchProfileRevision;

function hall(model_scale?: number | null): PatchedFixture {
	return {
		fixture_id: "fixture-hall",
		fixture_number: null,
		virtual_fixture_number: 1,
		name: "Hall",
		definition,
		universe: null,
		address: null,
		split_patches: [{ split: 1, universe: null, address: null }],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
		model_scale,
	} as PatchedFixture;
}

function roundTrip(fixture: PatchedFixture) {
	const { input } = patchedFixtureCandidate(fixture);
	return {
		input,
		read: projectionToPatchedFixture(
			{ ...input, fixtureRevision: 1, logicalHeads: [] },
			profile,
			() => definition,
		),
	};
}

describe("a Venue object's model scale through the Architect patch", () => {
	it("is written with every edit and read back unchanged", () => {
		const { input, read } = roundTrip(hall(2.5));
		expect(input.modelScale).toBe(2.5);
		expect(read.model_scale).toBe(2.5);
	});

	it("reads an object placed before the scale existed at its built size", () => {
		const { input, read } = roundTrip(hall());
		expect(input.modelScale).toBeNull();
		expect(read.model_scale).toBeNull();
	});
});
