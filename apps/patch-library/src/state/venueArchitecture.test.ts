import { describe, expect, it } from "vitest";
import type { PatchProfileRevision, VenueArchitecture } from "../contracts";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../sheet/fixtureProfileModel";
import {
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
	projectionToPatchedFixture,
} from "./model";

function arrange() {
	const source = blankFixtureProfile();
	source.direct_control_protocols = ["citp"];
	const definition = fixtureDefinitionsFromProfiles([source])[0];
	const fixture = newPatchFixtureCandidate({
		name: "Media A",
		fixture_number: 1,
		definition,
		universe: 51,
		address: 1,
	}).fixture;
	const profile: PatchProfileRevision = {
		profileId: source.id,
		profileRevision: source.revision,
		contentDigest: "",
		manufacturer: source.manufacturer,
		name: source.name,
		fixtureType: source.fixture_type,
		patchPolicy: "dmx",
		referencedModes: [],
		profileSnapshot: null,
	};
	return { fixture, profile, definition };
}

describe("TL843 venue construction placement transport", () => {
	const door: VenueArchitecture = {
		kind: "wall",
		ceiling_height_millimetres: 4000,
		openings: [
			{
				id: "02acbe86-d9d4-4d75-8ad0-2b4c6d2f374b",
				offset_millimetres: 500,
				sill_millimetres: 0,
				width_millimetres: 1000,
				height_millimetres: 2200,
				opening: {
					kind: "door",
					leaves: "double",
					swing: "outward",
					hinge: "right",
				},
			},
		],
	};
	it.each<VenueArchitecture>([
		door,
		{
			kind: "slab",
			purpose: "balcony",
			outline_millimetres: [
				{ x: 0, y: 0 },
				{ x: 6000, y: 0 },
				{ x: 6000, y: 2500 },
			],
			elevation_millimetres: 3000,
			thickness_millimetres: 200,
		},
		{ kind: "chair", style: "white" },
	])("preserves construction through reads, writes and moves: %j", (architecture) => {
		const { fixture, profile, definition } = arrange();
		fixture.scenery_options = { colour_srgb: "#445566", architecture };
		const input = patchedFixtureCandidate(fixture).input;
		expect(input.sceneryOptions?.architecture).toEqual(architecture);
		const read = projectionToPatchedFixture(
			{ ...input, fixtureRevision: 2, logicalHeads: [] },
			profile,
			() => definition,
		);
		expect(read.scenery_options?.architecture).toEqual(architecture);
		const moved = patchedFixtureCandidate({
			...read,
			location: { x: 3000, y: 4000, z: 0 },
		}).input;
		expect(moved.sceneryOptions?.architecture).toEqual(architecture);
		expect(moved.sceneryOptions?.colourSrgb).toBe("#445566");
	});
});
