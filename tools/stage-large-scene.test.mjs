import assert from "node:assert/strict";
import test from "node:test";
import {
	countFixtureInstances,
	createDeterministicLargeStageInputs,
	LARGE_STAGE_FIXTURE_INSTANCES,
	LARGE_STAGE_FIXTURE_RECORDS,
	LARGE_STAGE_MANIFEST,
} from "./stage-large-scene.mjs";

const footprints = new Map([
	["dls", 47],
	["wash", 37],
	["sunstrip", 30],
	["beam", 22],
	["dimmer", 1],
	["stage", 0],
	["stairs", 0],
	["truss", 0],
	["curtain", 0],
]);
const profiles = LARGE_STAGE_MANIFEST.map((entry) => ({
	id: `profile-${entry.key}`,
	revision: 1,
	manufacturer: entry.manufacturer,
	name: entry.name,
	modes: [
		{
			id: `mode-${entry.key}`,
			name: entry.mode,
			splits: [{ number: 1, footprint: footprints.get(entry.key) }],
		},
	],
}));

test("builds the exact realistic 970-record and 1,000-instance Stage profile", () => {
	const built = createDeterministicLargeStageInputs([], profiles);

	assert.equal(built.fixtures.length, LARGE_STAGE_FIXTURE_RECORDS);
	assert.equal(
		countFixtureInstances(built.fixtures),
		LARGE_STAGE_FIXTURE_INSTANCES,
	);
	assert.equal(built.addedMultipatchInstances, 30);
	assert.deepEqual(built.categoryCounts, {
		sunstrip: 40,
		moving: 500,
		static: 440,
		venue: 20,
	});
	assert.equal(built.dynamicFixtureIds.length, 540);
	assert.equal(built.staticControlFixtureIds.length, 410);
	assert.equal(built.patch.firstUniverse, 101);
	assert.equal(built.patch.lastUniverse, 137);
	assert.equal(built.patch.universeCount, 37);
	assert.equal(built.patch.occupiedSlots, 18_840);
	assert.equal(built.patch.occupiedByUniverse[137], 408);
	assert.equal(
		Object.values(built.patch.occupiedByUniverse).filter(
			(occupied) => occupied === 512,
		).length,
		36,
	);
	assert.ok(
		built.fixtures
			.flatMap((fixture) => [fixture, ...fixture.multipatch])
			.every((instance) =>
				instance.split_patches.every(
					(split) =>
						split.universe === null ||
						(split.address >= 1 && split.address <= 512),
				),
			),
	);
});

test("fails closed when a required shipped profile or exact mode is absent", () => {
	assert.throws(
		() => createDeterministicLargeStageInputs([], profiles.slice(1)),
		/ROBE Robin DLS Profile/,
	);
	assert.throws(
		() =>
			createDeterministicLargeStageInputs(
				[],
				profiles.map((profile) =>
					profile.name === "Robin DLS Profile"
						? { ...profile, modes: [] }
						: profile,
				),
			),
		/Robin DLS Profile \/ Mode 1/,
	);
});
