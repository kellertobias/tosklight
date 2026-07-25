import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../src/api/types";
import { discreteChoicesForSelection } from "./discreteEncoderScenario";

describe("profile-derived discrete encoder catalog", () => {
	it("exposes stable semantic choices only for compatible selected fixtures", () => {
		const compatible = fixture("fixture-a", "head-a", [
			["gobo.open", "Open"],
			["gobo.dots", "Dots"],
		]);
		const incompatible = fixture("fixture-b", "head-b", [], "intensity");

		expect(
			discreteChoicesForSelection(
				[compatible, incompatible],
				["fixture-a", "fixture-b"],
				"gobo",
			),
		).toEqual([
			{ semanticId: "gobo.dots", label: "Dots" },
			{ semanticId: "gobo.open", label: "Open" },
		]);
		expect(
			discreteChoicesForSelection(
				[compatible, incompatible],
				["fixture-b"],
				"gobo",
			),
		).toEqual([]);
	});

	it("resolves a logical head by stable profile head identity", () => {
		const profile = fixture("fixture-a", "logical-a", [["prism.3", "3 Facet"]]);
		profile.logical_heads = [
			{
				profile_head_id: "profile-head",
				fixture_id: "logical-a",
				head_index: 0,
			},
		];

		expect(
			discreteChoicesForSelection([profile], ["logical-a"], "gobo"),
		).toEqual([{ semanticId: "prism.3", label: "3 Facet" }]);
	});
});

function fixture(
	fixtureId: string,
	logicalFixtureId: string,
	choices: Array<[string, string]>,
	attribute = "gobo",
): PatchedFixture {
	return {
		fixture_id: fixtureId,
		universe: 1,
		address: 1,
		logical_heads: [
			{
				profile_head_id: "profile-head",
				fixture_id: logicalFixtureId,
				head_index: 0,
			},
		],
		definition: {
			id: "definition",
			revision: 1,
			manufacturer: "Bench",
			model: "Discrete",
			name: "Discrete",
			mode: "Mode",
			device_type: "moving_head",
			footprint: 1,
			physical: {
				width_millimetres: null,
				height_millimetres: null,
				depth_millimetres: null,
				weight_kilograms: null,
				power_watts: null,
			},
			heads: [],
			color_calibration: null,
			hazardous: false,
			safe_values: {},
			profile_id: "profile",
			mode_id: "mode",
			profile_snapshot: {
				schema_version: 2,
				id: "profile",
				revision: 1,
				manufacturer: "Bench",
				name: "Discrete",
				short_name: "Discrete",
				fixture_type: "moving_head",
				notes: "",
				photograph_asset: null,
				stage_icon_asset: null,
				model_asset: null,
				physical: {
					width_millimetres: null,
					height_millimetres: null,
					depth_millimetres: null,
					weight_kilograms: null,
					power_watts: null,
				},
				modes: [
					{
						id: "mode",
						name: "Mode",
						notes: "",
						splits: [{ number: 1, footprint: 1 }],
						heads: [
							{ id: "profile-head", name: "Main", master_shared: false },
						],
						channels: [
							{
								id: "channel",
								head_id: "profile-head",
								split: 1,
								attribute,
								resolution: "u8",
								secondary_slots: [],
								default_raw: 0,
								highlight_raw: 0,
								physical_min: null,
								physical_max: null,
								unit: null,
								invert: false,
								snap: true,
								reacts_to_virtual_intensity: false,
								reacts_to_sequence_master: false,
								reacts_to_group_master: false,
								reacts_to_grand_master: false,
								behavior: "controlled",
								functions: choices.map(([semanticId, label], index) => ({
									id: `function-${index}`,
									name: label,
									dmx_from: index,
									dmx_to: index,
									attribute,
									priority: 0,
									behavior: {
										type: "indexed",
										semantic_id: semanticId,
										label,
										raw_value: index,
									},
								})),
							},
						],
						color_systems: [],
						control_actions: [],
						geometry: { nodes: [], emitters: [] },
					},
				],
				hazardous: false,
				direct_control_protocols: [],
				signal_loss_policy: { type: "hold_last" },
				reserved_source: null,
			},
		},
	};
}
