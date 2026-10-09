import { describe, expect, it } from "vitest";
import type {
	AttributeDescriptor,
	FixtureDefinition,
	PatchedFixture,
	VisualizationSnapshot,
} from "../api/types";
import type { AttributeValue } from "../api/types/playback";
import { fixtureSheetSnapshotsEqual } from "./fixtureSheetProjection";
import { fixtureSheetTargets } from "./fixtureSheetTargets";
import {
	FIXTURE_SHEET_ATTRIBUTE_GROUPS,
	fixtureSheetGroupValues,
	fixtureSheetValueIndex,
	withCommandedPositions,
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
	it("FIXTURE-SHEET-002-002 keeps all eight groups, semantic media pairs, and separate Dynamic identities", () => {
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

	it("keeps an ordinary winning contribution's source even when its value equals the default", () => {
		const target = fixtureSheetTargets(fixture())[0];
		const values = new Map([
			["intensity", { kind: "normalized" as const, value: 0 }],
		]);
		const groups = fixtureSheetGroupValues({
			target,
			registry,
			values,
			preloadValues: undefined,
			programmerAttributes: new Set(),
			dynamicStack: [],
			preloadDynamicStack: [],
		});
		expect(groups.intensity.members[0]).toMatchObject({
			text: "0%",
			source: "playback",
		});
	});
	it("shows native Red/Green/Blue output when the semantic registry publishes no Color descriptors", () => {
		const target = fixtureSheetTargets(fixture())[0];
		const semanticRegistry = registry.filter(
			(descriptor) => !descriptor.id.startsWith("color."),
		);
		const groups = fixtureSheetGroupValues({
			target,
			registry: semanticRegistry,
			values: new Map([
				["color.red", { kind: "normalized" as const, value: 0.5 }],
			]),
			preloadValues: undefined,
			programmerAttributes: new Set(["color"]),
			dynamicStack: [],
			preloadDynamicStack: [],
		});
		expect(groups.color.available).toBe(true);
		expect(groups.color.members[0]).toMatchObject({
			attribute: "color.red",
			label: "Red",
			text: "50%",
			source: "programmer",
		});
	});
	describe("Position reads the commanded pose in degrees (TL-552)", () => {
		// Production Pan/Tilt descriptors carry degrees without a channel domain.
		const angleRegistry = registry.map((descriptor) =>
			descriptor.id === "pan"
				? { ...descriptor, domain_min: null, domain_max: null }
				: descriptor,
		);
		const empty = (): VisualizationSnapshot => ({
			...snapshot("2026-08-02T10:00:00Z"),
			values: [],
			dynamic_stack: [],
		});
		const commanded = (
			snapshot: VisualizationSnapshot,
			rows: VisualizationSnapshot["commanded_positions"],
		) => withCommandedPositions({ ...snapshot, commanded_positions: rows });
		const pan = (
			current: VisualizationSnapshot | null,
			programmerAttributes = new Set<string>(),
		) =>
			fixtureSheetGroupValues({
				target: fixtureSheetTargets(fixture())[0],
				registry: angleRegistry,
				values: fixtureSheetValueIndex(current).get("fixture-1"),
				preloadValues: undefined,
				programmerAttributes,
				dynamicStack: [],
				preloadDynamicStack: [],
			}).position.members[0];

		it("never labels a channel fraction as degrees", () => {
			expect(pan(empty()).text).toBe("—");
		});

		it("shows an idle mover's commanded default pose as the encoders do", () => {
			// DMX 128/255 on a nominal 540° Pan travel: the encoder reads Pan 1.1°.
			const shown = commanded(empty(), [
				{
					fixture_id: "fixture-1",
					pan_degrees: 1.0588236,
					tilt_degrees: 0.5294118,
				},
			]);
			expect(pan(shown)).toMatchObject({ text: "1.1°", source: "default" });
		});

		it("prefers the commanded pose over the requested Angles and keeps the Programmer source", () => {
			const requested = empty();
			requested.values = [
				{
					fixture_id: "fixture-1",
					attribute: "position",
					value: {
						kind: "position",
						value: {
							kind: "angles",
							pan_degrees: { kind: "value", value: 30 },
							tilt_degrees: { kind: "value", value: 10 },
						},
					},
				},
			];
			const programmed = new Set(["position"]);
			expect(pan(requested, programmed)).toMatchObject({
				text: "30°",
				source: "programmer",
			});
			const shown = commanded(requested, [
				{ fixture_id: "fixture-1", pan_degrees: 29.5, tilt_degrees: 10 },
			]);
			expect(pan(shown, programmed).text).toBe("29.5°");
		});

		it("adds nothing when the server lists no common pose or there is no snapshot", () => {
			// Divergent copies and owners without a pose are absent from the server's rows.
			expect(pan(commanded(empty(), [])).text).toBe("—");
			expect(withCommandedPositions(null)).toBeNull();
		});
	});
});

describe("whole Color family presentation", () => {
	it("reads retained Direct identity instead of inventing native scalar zeroes", () => {
		const value = directFamily();
		const values = fixtureSheetValueIndex({
			...snapshot("now"),
			values: [{ fixture_id: "fixture-1", attribute: "color", value }],
		}).get("fixture-1");
		const groups = fixtureSheetGroupValues({
			target: fixtureSheetTargets(fixture())[0],
			registry,
			values,
			preloadValues: undefined,
			programmerAttributes: new Set(),
			dynamicStack: [],
			preloadDynamicStack: [],
		});
		expect(groups.color.members).toHaveLength(1);
		expect(groups.color.members[0]).toMatchObject({
			attribute: "color",
			value,
			text: "Direct · unknown appearance",
			source: "playback",
		});
		expect(groups.color.accessibleName).toContain("profile");
		expect(value.value).toMatchObject({
			recipe: { channels: [{ raw: 32 }, { raw: 64 }] },
		});
	});
});

function directFamily(): AttributeValue {
	return {
		kind: "color_program",
		value: {
			kind: "direct",
			recipe: {
				source: {
					profile_id: "profile",
					profile_revision: 1,
					mode_id: "full",
					head_id: "main",
					path_id: "main",
					profile_digest: "digest",
					native_layout_signature: "layout",
					model_revision: 0,
				},
				channels: [
					{ channel_id: "red", function_id: "red", raw: 32 },
					{ channel_id: "blue", function_id: "blue", raw: 64 },
				],
			},
			portable: {
				model_revision: 0,
				visible: null,
				uv: null,
				quality: "unknown",
				limitations: [],
			},
		},
	};
}

describe("Color family lanes and identities", () => {
	function group(
		values: Map<string, AttributeValue> | undefined,
		pending?: Map<string, AttributeValue>,
	) {
		return fixtureSheetGroupValues({
			target: fixtureSheetTargets(fixture())[0],
			registry,
			values,
			preloadValues: pending,
			programmerAttributes: new Set(),
			dynamicStack: [],
			preloadDynamicStack: [],
		}).color;
	}
	it("leaves absent Normal unavailable while preserving pending Direct, and omits identical pending", () => {
		const direct = directFamily();
		const pending = group(undefined, new Map([["color", direct]]));
		expect(pending.members[0]).toMatchObject({
			value: null,
			text: "Unavailable",
			source: "default",
			preloadValue: direct,
			preloadText: "Direct · unknown appearance",
		});
		expect(
			group(new Map([["color", direct]]), new Map([["color", direct]]))
				.members[0].preloadValue,
		).toBeNull();
	});
	it("keeps family Dynamic identity and ordinary base despite changing sampled values", () => {
		const direct = directFamily();
		const dynamic = {
			...snapshot("now").dynamic_stack![0],
			attribute: "color",
			entry_type: "dynamic" as const,
			value: { kind: "normalized" as const, value: 0.7 },
		};
		const options = {
			target: fixtureSheetTargets(fixture())[0],
			registry,
			values: new Map([["color", direct]]),
			preloadValues: undefined,
			programmerAttributes: new Set(["color"]),
			dynamicStack: [dynamic],
			preloadDynamicStack: [{ ...dynamic, pending: true }],
		};
		const first = fixtureSheetGroupValues(options).color;
		const second = fixtureSheetGroupValues({
			...options,
			dynamicStack: [{ ...dynamic, value: { kind: "normalized", value: 0.1 } }],
		}).color;
		expect(first.members[0]).toMatchObject({
			value: direct,
			source: "programmer",
		});
		expect(first.members[0].dynamics.map((item) => item.lane)).toEqual([
			"normal",
			"preload",
		]);
		expect(first.members[0].text).toEqual(second.members[0].text);
	});
	it("retains a family Dynamic identity even when no ordinary Color base exists", () => {
		const dynamic = {
			...snapshot("now").dynamic_stack![0],
			attribute: "color",
			entry_type: "dynamic" as const,
		};
		const result = fixtureSheetGroupValues({
			target: fixtureSheetTargets(fixture())[0],
			registry,
			values: undefined,
			preloadValues: undefined,
			programmerAttributes: new Set(),
			dynamicStack: [dynamic],
			preloadDynamicStack: [],
		}).color;
		expect(result.members[0]).toMatchObject({
			attribute: "color",
			value: null,
			text: "Unavailable",
		});
		expect(result.members[0].dynamics).toHaveLength(1);
	});

	it("indexes separate logical owners without borrowing a parent's family", () => {
		const direct = directFamily();
		const indexed = fixtureSheetValueIndex({
			...snapshot("now"),
			values: [
				{ fixture_id: "root", attribute: "color", value: direct },
				{
					fixture_id: "head",
					attribute: "color",
					value: { kind: "color_xyz", value: { x: 0, y: 0, z: 0 } },
				},
			],
		});
		expect(group(indexed.get("head")).members[0].text).toBe("Color");
		expect(indexed.get("other")).toBeUndefined();
	});
});

it("preserves actual legacy RGB scalars on a target that also advertises Color", () => {
	const patched = fixture();
	patched.definition.heads[0].parameters.push({
		...patched.definition.heads[0].parameters[1],
		attribute: "color",
	});
	const target = fixtureSheetTargets(patched)[0];
	const values = new Map<string, AttributeValue>([
		["color.red", { kind: "normalized", value: 0.5 }],
	]);
	const options = {
		target,
		registry,
		values,
		preloadValues: undefined,
		programmerAttributes: new Set<string>(),
		dynamicStack: [],
		preloadDynamicStack: [],
	};
	const legacy = fixtureSheetGroupValues(options).color;
	expect(
		legacy.members.find((member) => member.attribute === "color.red"),
	).toMatchObject({
		value: { kind: "normalized", value: 0.5 },
		text: "50%",
		source: "playback",
	});
	expect(legacy.members.some((member) => member.attribute === "color")).toBe(
		false,
	);
	const dynamic = {
		...snapshot("now").dynamic_stack![0],
		attribute: "color",
		entry_type: "dynamic" as const,
	};
	const dynamicLegacy = fixtureSheetGroupValues({
		...options,
		dynamicStack: [dynamic],
	}).color;
	expect(dynamicLegacy.members[0]).toMatchObject({
		attribute: "color.red",
		text: "50%",
	});
	expect(dynamicLegacy.members[0].dynamics[0].attribute).toBe("color");
	const pendingScalar = fixtureSheetGroupValues({
		...options,
		values: undefined,
		preloadValues: values,
	}).color;
	expect(pendingScalar.members[0]).toMatchObject({
		attribute: "color.red",
		preloadText: "50%",
	});

	const whole = fixtureSheetGroupValues({
		...options,
		values: new Map([...values, ["color", directFamily()]]),
	}).color;
	expect(whole.members).toHaveLength(1);
	expect(whole.members[0].text).toBe("Direct · unknown appearance");
	const pending = fixtureSheetGroupValues({
		...options,
		preloadValues: new Map([["color", directFamily()]]),
	}).color;
	expect(pending.members[0]).toMatchObject({
		attribute: "color",
		value: null,
		preloadText: "Direct · unknown appearance",
	});
});
