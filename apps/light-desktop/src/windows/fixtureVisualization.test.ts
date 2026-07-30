import { describe, expect, it } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import {
	fixtureProfileOutputValue,
	fixtureValue,
} from "./fixtureVisualization";
import { fixturePresentation } from "./stageWindow/useStageVisualization";

const fixture = {
	fixture_id: "physical",
	universe: 1,
	address: 1,
	logical_heads: [{ fixture_id: "head-1", head_index: 1 }],
	definition: {
		schema_version: 1,
		id: "definition",
		revision: 1,
		manufacturer: "Test",
		device_type: "moving light",
		name: "Mover",
		model: "Mover",
		mode: "Mode",
		footprint: 2,
		color_calibration: null,
		physical: {},
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
		heads: [
			{
				index: 1,
				name: "Head",
				shared: false,
				parameters: [
					{
						attribute: "intensity",
						components: [],
						default: 0.15,
						virtual_dimmer: false,
						capabilities: [],
					},
					{
						attribute: "pan",
						components: [],
						default: 0.35,
						virtual_dimmer: false,
						capabilities: [],
					},
				],
			},
		],
	},
} satisfies PatchedFixture;

describe("fixture visualization values", () => {
	it("uses fixture defaults instead of demo state when output has no contribution", () => {
		expect(fixtureValue(null, fixture, "intensity")).toBe(0.15);
		expect(fixtureValue(null, fixture, "pan")).toBe(0.35);
	});

	it("resolves a logical head's live value for its physical lamp", () => {
		const snapshot = {
			revision: 1,
			generated_at: "2026-07-12T00:00:00Z",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: "head-1",
					attribute: "pan",
					value: { kind: "normalized", value: 0.8 },
				},
			],
		} satisfies VisualizationSnapshot;
		expect(fixtureValue(snapshot, fixture, "pan")).toBe(0.8);
	});

	it("preserves the first matching snapshot entry across a fixture family", () => {
		const snapshot = {
			revision: 1,
			generated_at: "2026-07-12T00:00:00Z",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: "head-1",
					attribute: "pan",
					value: { kind: "normalized", value: 0.8 },
				},
				{
					fixture_id: "physical",
					attribute: "pan",
					value: { kind: "normalized", value: 0.2 },
				},
			],
		} satisfies VisualizationSnapshot;
		expect(fixtureValue(snapshot, fixture, "pan")).toBe(0.8);
	});

	it("uses authoritative post-profile intensity for per-fixture master policies", () => {
		const snapshot = {
			revision: 1,
			generated_at: "2026-07-12T00:00:00Z",
			grand_master: 0.2,
			blackout: false,
			values: [
				{
					fixture_id: "physical",
					attribute: "intensity",
					value: { kind: "normalized", value: 1 },
				},
			],
			profile_output_values: [
				{
					fixture_id: "physical",
					attribute: "intensity",
					value: { kind: "normalized", value: 1 },
				},
			],
		} satisfies VisualizationSnapshot;
		expect(fixtureProfileOutputValue(snapshot, fixture, "intensity")).toBe(1);
		expect(fixturePresentation(fixture, 0, snapshot, false).dimmer).toBe(100);
	});
});
