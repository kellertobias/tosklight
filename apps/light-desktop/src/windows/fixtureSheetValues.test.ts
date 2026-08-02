import { describe, expect, it } from "vitest";
import type {
	AttributeDescriptor,
	FixtureDefinition,
	PatchedFixture,
	VisualizationSnapshot,
} from "../api/types";
import { fixtureSheetSnapshotsEqual } from "./fixtureSheetProjection";
import { fixtureSheetTargets } from "./fixtureSheetTargets";
import {
	FIXTURE_SHEET_ATTRIBUTE_GROUPS,
	fixtureSheetGroupValues,
	fixtureSheetValueIndex,
} from "./fixtureSheetValues";

const attributeGroups = [
	["intensity", "Intensity", "intensity", "percent"],
	["color.red", "Red", "color", "percent"],
	["pan", "Pan", "position", "deg"],
	["gobo", "Gobo", "beam", null],
	["shaper.blade.1.position", "Blade 1", "shapers", "percent"],
	["focus", "Focus", "focus", "percent"],
	["control.mode", "Fixture Mode", "control", null],
	["media.folder", "Media Folder", "media", null],
	["media.file", "Media File", "media", null],
	["media.mask.folder", "Mask Folder", "media", null],
	["media.mask.file", "Mask File", "media", null],
] as const;

const registry: AttributeDescriptor[] = attributeGroups.map(
	([id, label, encoder_group, display_unit], index) => ({
		id,
		label,
		family: encoder_group,
		value_type:
			id === "gobo" || id.includes("folder") || id.includes("file")
				? "indexed"
				: "continuous",
		default_unit: display_unit,
		display_unit,
		domain_min: id === "pan" ? -270 : null,
		domain_max: id === "pan" ? 270 : null,
		encoder_group,
		encoder_page: 1,
		encoder_slot: index + 1,
		retired: false,
	}),
);

function fixture(): PatchedFixture {
	return {
		fixture_id: "fixture-1",
		fixture_number: 1,
		name: "Media Profile",
		universe: 1,
		address: 1,
		definition: {
			schema_version: 1,
			id: "definition",
			revision: 1,
			manufacturer: "Test",
			device_type: "fixture",
			name: "Fixture",
			model: "Fixture",
			mode: "Full",
			mode_id: "full",
			footprint: 16,
			heads: [
				{
					index: 0,
					name: "Main",
					shared: true,
					parameters: attributeGroups.map(([attribute]) => ({
						attribute,
						components: [],
						default: 0,
						virtual_dimmer: false,
						capabilities: [],
					})),
				},
			],
			profile_snapshot: {
				schema_version: 2,
				id: "profile",
				revision: 1,
				manufacturer: "Test",
				name: "Fixture",
				short_name: "Fixture",
				fixture_type: "profile",
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
						id: "full",
						name: "Full",
						notes: "",
						splits: [],
						heads: [],
						channels: [
							{
								id: "gobo",
								head_id: "main",
								split: 1,
								fixture_attribute: "gobo",
								attribute: "gobo",
								canonical_transform: "identity",
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
								functions: [
									{
										id: "dots",
										name: "Dots",
										dmx_from: 1,
										dmx_to: 1,
										attribute: "gobo",
										priority: 0,
										behavior: {
											type: "indexed",
											semantic_id: "gobo.dots",
											label: "Gobo Dots",
											raw_value: 1,
										},
									},
								],
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
			color_calibration: null,
			physical: {},
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
		} as FixtureDefinition,
		logical_heads: [],
	};
}

function snapshot(generatedAt: string, red = 0.25): VisualizationSnapshot {
	return {
		scope: { show_id: "show-1" },
		revision: 4,
		generated_at: generatedAt,
		grand_master: 1,
		blackout: false,
		values: [
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
			},
			{
				fixture_id: "fixture-1",
				attribute: "color.red",
				value: { kind: "normalized", value: red },
			},
			{
				fixture_id: "fixture-1",
				attribute: "pan",
				value: { kind: "normalized", value: 0.75 },
			},
			{
				fixture_id: "fixture-1",
				attribute: "gobo",
				value: { kind: "discrete", value: "gobo.dots" },
			},
			...[
				["media.folder", "2"],
				["media.file", "7"],
				["media.mask.folder", "1"],
				["media.mask.file", "4"],
			].map(([attribute, value]) => ({
				fixture_id: "fixture-1",
				attribute: attribute ?? "",
				value: { kind: "discrete" as const, value: value ?? "" },
			})),
		],
		dynamic_stack: [
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				entry_type: "dynamic",
				priority: 10,
				changed_at_millis: 1,
				source: "Programmer",
				dynamic_id: "dynamic-1",
				pool_number: 7,
				name: "Pulse",
				paused: false,
				hidden: false,
				pending: false,
				winning: true,
			},
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				entry_type: "dynamic",
				priority: 5,
				changed_at_millis: 2,
				source: "Cue 1",
				dynamic_id: "dynamic-2",
				pool_number: 8,
				name: "Sine",
				paused: true,
				hidden: true,
				pending: false,
				winning: false,
			},
		],
	};
}

describe("Fixture Sheet attribute-group values", () => {
	it("keeps all eight groups, semantic media pairs, and separate Dynamic identities", () => {
		const target = fixtureSheetTargets(fixture())[0];
		const current = snapshot("2026-08-02T10:00:00Z");
		const preload = snapshot("2026-08-02T10:00:01Z");
		preload.preload = true;
		preload.values[0] = {
			fixture_id: "fixture-1",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.8 },
		};
		preload.dynamic_stack = [
			{
				...current.dynamic_stack?.[0],
				fixture_id: "fixture-1",
				attribute: "gobo",
				entry_type: "dynamic",
				priority: 10,
				changed_at_millis: 3,
				source: "Preload",
				dynamic_id: null,
				pool_number: null,
				name: "Recorded look",
				runtime_instance_id: "snapshot-abcdef12",
				paused: false,
				hidden: false,
				pending: true,
				winning: true,
			},
		];
		const groups = fixtureSheetGroupValues({
			target,
			registry,
			values: fixtureSheetValueIndex(current).get("fixture-1"),
			preloadValues: fixtureSheetValueIndex(preload).get("fixture-1"),
			programmerAttributes: new Set(["intensity"]),
			dynamicStack: current.dynamic_stack ?? [],
			preloadDynamicStack: preload.dynamic_stack ?? [],
		});

		expect(Object.keys(groups)).toEqual(FIXTURE_SHEET_ATTRIBUTE_GROUPS);
		expect(groups.intensity.members[0]).toMatchObject({
			text: "50%",
			preloadText: "80%",
			source: "programmer",
		});
		expect(
			groups.intensity.members[0].dynamics.map((dynamic) => dynamic.label),
		).toEqual(["7", "8"]);
		expect(groups.intensity.members[0].dynamics[1].accessibleName).toContain(
			"paused, hidden, non-winning",
		);
		expect(groups.position.members[0].text).toBe("135°");
		expect(groups.beam.members[0].text).toBe("Gobo Dots");
		expect(groups.beam.members[0].dynamics[0].label).toBe("Snapshot snapshot");
		expect(
			groups.media.members.map(({ label, text }) => [label, text]),
		).toEqual([
			["Media Folder", "2"],
			["Media File", "7"],
			["Mask Folder", "1"],
			["Mask File", "4"],
		]);
		expect(groups.shapers.available).toBe(true);
		expect(groups.focus.available).toBe(true);
		expect(groups.control.available).toBe(true);
	});

	it("ignores transport timestamps and sampled-only fields when deciding to repaint", () => {
		const left = snapshot("2026-08-02T10:00:00Z");
		const right = snapshot("2026-08-02T10:00:02Z");
		right.revision = 99;
		if (right.dynamic_stack?.[0]) {
			right.dynamic_stack[0].value = { kind: "normalized", value: 0.9 };
			right.dynamic_stack[0].resolved_value = {
				kind: "normalized",
				value: 0.1,
			};
		}
		expect(fixtureSheetSnapshotsEqual(left, right)).toBe(true);
		expect(
			fixtureSheetSnapshotsEqual(left, snapshot("2026-08-02T10:00:03Z", 0.8)),
		).toBe(false);
	});
});
