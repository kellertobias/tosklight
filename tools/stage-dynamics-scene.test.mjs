import assert from "node:assert/strict";
import test from "node:test";
import { validateSemanticDefinition } from "./semantic-performance-contract.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";

/** A lane's address: the scalar attribute, or the semantic owner component (TL-552). */
const laneAddress = (lane) =>
	lane.attribute ??
	(lane.programming.address.representation.kind === "angles"
		? `position.${lane.programming.address.component.kind}`
		: `color.${lane.programming.address.component.component}`);

/** No lane may author the legacy scalar Color or Position addresses refused at contract 1. */
function assertContractOneLanes(plan) {
	for (const definition of plan.definitions) {
		for (const lane of definition.lanes) {
			if (lane.attribute) assert.equal(lane.attribute, "intensity");
		}
		const typed = { ...definition, lanes: definition.lanes.filter((lane) => lane.programming) };
		if (typed.lanes.length > 0)
			assert.deepEqual(validateSemanticDefinition(typed), [], definition.name);
	}
}

test("expands logical pixel owners and partitions exact addresses into 20 Dynamics", () => {
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
	assert.equal(plan.dynamicTargetCount, 84);
	assert.deepEqual(plan.staticControlFixtureIds, ["fixed-dimmer"]);
	const targets = plan.activations.flatMap((activation) => activation.targets);
	assert.equal(new Set(targets).size, 21);
	assert.ok(targets.every((target) => target.startsWith("pixel-")));
	assert.ok(
		plan.definitions.every(
			(definition) => definition.target_binding.type === "targetless",
		),
	);
	// One Dynamic animates one family: intensity alone, or the whole RGB recipe of its pixels.
	assert.deepEqual(
		new Set(
			plan.definitions.map((definition) =>
				definition.lanes.map(laneAddress).join("|"),
			),
		),
		new Set(["intensity", "color.blue|color.green|color.red"]),
	);
	assert.deepEqual(plan.laneCoverage, {
		intensity: 21,
		"color.red": 21,
		"color.green": 21,
		"color.blue": 21,
	});
	assertContractOneLanes(plan);
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
	assert.deepEqual(
		new Set(
			plan.definitions.flatMap((definition) =>
				definition.lanes.map(laneAddress),
			),
		),
		new Set([
			"intensity",
			"position.pan",
			"position.tilt",
			"color.red",
			"color.green",
			"color.blue",
		]),
	);
	// Pan and Tilt run as one Angle pair; the 20–80 % sweep is ±162°/±81° on a centred travel.
	const angles = plan.definitions.find((definition) =>
		definition.lanes.some((lane) => lane.programming?.address.representation.kind === "angles"),
	);
	assert.deepEqual(angles.lanes.map(laneAddress), ["position.pan", "position.tilt"]);
	assert.deepEqual(
		angles.lanes.map((lane) => [
			lane.programming.configuration.configuration.minimum.value.value,
			lane.programming.configuration.configuration.maximum.value.value,
		]),
		[
			[-162, 162],
			[-81, 81],
		],
	);
	assert.equal(plan.dynamicTargetCount, 20 * 6);
	assertContractOneLanes(plan);
});
