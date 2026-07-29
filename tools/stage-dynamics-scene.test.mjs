import assert from "node:assert/strict";
import test from "node:test";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";

test("expands logical pixel owners and partitions exact lane signatures into 20 Dynamics", () => {
	const fixtures = Array.from({ length: 21 }, (_, index) => ({
		fixture_id: `fixture-${index}`,
		name: `Sunstrip ${index}`,
		profile_id: "sunstrip-profile",
		profile_revision: 1,
		mode_id: "sunstrip-mode",
		logical_heads: [
			{
				profile_head_id: "pixel",
				head_index: 0,
				fixture_id: `pixel-${index}`,
			},
		],
	}));
	const patch = {
		fixtures,
		profile_revisions: [
			{
				profile_id: "sunstrip-profile",
				profile_revision: 1,
				profile_snapshot: {
					modes: [
						{
							id: "sunstrip-mode",
							heads: [
								{
									id: "pixel",
									name: "Pixel",
									master_shared: false,
								},
							],
							channels: ["red", "green", "blue"].map((color) => ({
								head_id: "pixel",
								attribute: `color.${color}`,
								reacts_to_virtual_intensity: true,
							})),
						},
					],
				},
			},
		],
	};
	const plan = createLargeStageDynamicsPlan(patch, {
		dynamicFixtureIds: fixtures.map((fixture) => fixture.fixture_id),
		staticControlFixtureIds: ["fixed-dimmer"],
	});

	assert.equal(plan.definitions.length, 20);
	assert.equal(plan.dynamicTargetCount, 21);
	assert.deepEqual(plan.staticControlFixtureIds, ["fixed-dimmer"]);
	const targets = plan.definitions.flatMap(
		(definition) => definition.target_binding.targets,
	);
	assert.equal(new Set(targets).size, 21);
	assert.ok(targets.every((target) => target.startsWith("pixel-")));
	assert.ok(
		plan.definitions.every(
			(definition) =>
				definition.lanes
					.map((lane) => lane.attribute)
					.sort()
					.join("|") === "color.blue|color.green|color.red|intensity",
		),
	);
});

test("keeps root moving-head attributes together and fixed dimmers excluded", () => {
	const fixture = {
		fixture_id: "mover",
		name: "Mover",
		profile_id: "mover-profile",
		profile_revision: 1,
		mode_id: "mover-mode",
		logical_heads: [],
	};
	const patch = {
		fixtures: Array.from({ length: 20 }, (_, index) => ({
			...fixture,
			fixture_id: `mover-${index}`,
		})),
		profile_revisions: [
			{
				profile_id: "mover-profile",
				profile_revision: 1,
				profile_snapshot: {
					modes: [
						{
							id: "mover-mode",
							heads: [{ id: "main", name: "Main", master_shared: true }],
							channels: [
								"intensity",
								"pan",
								"tilt",
								"color.red",
								"color.green",
								"color.blue",
								"color.wheel.1",
							].map((attribute) => ({
								head_id: "main",
								attribute,
								reacts_to_virtual_intensity: false,
							})),
						},
					],
				},
			},
		],
	};
	const plan = createLargeStageDynamicsPlan(patch, {
		dynamicFixtureIds: patch.fixtures.map((item) => item.fixture_id),
		staticControlFixtureIds: ["dimmer"],
	});

	assert.equal(plan.definitions.length, 20);
	assert.equal(
		plan.definitions[0].lanes.some(
			(lane) => lane.attribute === "color.wheel.1",
		),
		false,
	);
	assert.deepEqual(
		new Set(plan.definitions[0].lanes.map((lane) => lane.attribute)),
		new Set([
			"intensity",
			"pan",
			"tilt",
			"color.red",
			"color.green",
			"color.blue",
		]),
	);
});
