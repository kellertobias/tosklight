import { describe, expect, it } from "vitest";
import type { PatchProfileRevision } from "../contracts";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../sheet/fixtureProfileModel";
import {
	changedPatchFixtureCandidate,
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

describe("TL738 Architect shared Patch endpoint persistence", () => {
	it.each([
		18080,
		18081,
		null,
		undefined,
	])("preserves HTTP port %s through a placement write", (httpPort) => {
		const { fixture, profile, definition } = arrange();
		Object.assign(fixture, {
			direct_control: {
				protocol: "citp",
				ip_address: "127.0.0.1",
				port: 14809,
				...(httpPort === undefined ? {} : { http_port: httpPort }),
			},
		});
		const input = patchedFixtureCandidate(fixture).input;
		expect(input.directControl?.httpPort).toBe(httpPort ?? null);
		const read = projectionToPatchedFixture(
			{ ...input, fixtureRevision: 1, logicalHeads: [] },
			profile,
			() => definition,
		);
		expect(read.direct_control).toMatchObject({
			port: 14809,
			http_port: httpPort ?? null,
		});
		const moved = patchedFixtureCandidate({
			...read,
			location: { x: 2000, y: 0, z: 0 },
		}).input;
		expect(moved.directControl?.httpPort).toBe(httpPort ?? null);
		expect(moved.location.x).toBe(2000);
	});
});


it("preserves explicit root/copy placement through projected rename, address and rotation writes", () => {
	const { fixture, profile, definition } = arrange();
	fixture.stage_position_set = false;
	fixture.multipatch = [{ id: "copy", name: "Copy", universe: null, address: null,
		location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 }, stage_position_set: true }];
	const input = patchedFixtureCandidate(fixture).input;
	const read = projectionToPatchedFixture({ ...input, fixtureRevision: 1, logicalHeads: [] }, profile, () => definition);
	const renamed = changedPatchFixtureCandidate(read, { name: "Renamed", address: 2, rotation: { x: 0, y: 0, z: 90 } });
	expect(renamed.input.stagePositionSet).toBe(false);
	expect(renamed.input.multipatch[0].stagePositionSet).toBe(true);
	expect(renamed.fixture.multipatch?.[0].stage_position_set).toBe(true);
	const placed = changedPatchFixtureCandidate(read, { location: { x: 0, y: 0, z: 0 } });
	expect(placed.input.stagePositionSet).toBe(true);
});
