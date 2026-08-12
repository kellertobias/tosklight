import { describe, expect, it } from "vitest";
import type { DmxSnapshot, VisualizationSnapshot } from "../api/types";
import {
	createVisualizationWidget,
	mixedColor,
	normalizeVisualizationRows,
	resolveVisualizationValue,
} from "./visualizationPaneModel";

const dmx: DmxSnapshot = {
	revision: 1,
	universes: [{ universe: 2, slots: [0, 128, 255] }],
	overrides: [],
};

const visualization: VisualizationSnapshot = {
	revision: 2,
	generated_at: "2026-08-12T00:00:00Z",
	grand_master: 1,
	blackout: false,
	values: [
		{
			fixture_id: "fixture-1",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.75 },
		},
	],
};

describe("visualization pane model", () => {
	it("resolves raw DMX and fixture attributes through scale, processing, and bounds", () => {
		const raw = {
			...createVisualizationWidget("raw"),
			source: { kind: "raw_dmx" as const, universe: 2, address: 2 },
			displayScale: "percent" as const,
			operation: "multiply" as const,
			factor: 2,
			maximum: 80,
		};
		expect(resolveVisualizationValue(raw, dmx, visualization)).toBe(80);

		const fixture = {
			...createVisualizationWidget("fixture"),
			source: {
				kind: "fixture_attribute" as const,
				fixtureId: "fixture-1",
				attribute: "intensity",
			},
			displayScale: "dmx" as const,
			operation: "divide" as const,
			factor: 3,
			maximum: 255,
		};
		expect(resolveVisualizationValue(fixture, dmx, visualization)).toBe(63.75);
	});

	it("normalizes persisted rows and rejects unsafe future values", () => {
		const [row] = normalizeVisualizationRows([
			{
				id: "row",
				widgets: [
					{
						...createVisualizationWidget("widget"),
						type: "future-widget",
						minimum: 10,
						maximum: 5,
						source: { kind: "raw_dmx", universe: -1, address: 900 },
						graph: { timeWindowSeconds: 999_999, lineLowColor: "red" },
					},
				],
			},
		]);
		expect(row.widgets[0]).toMatchObject({
			type: "number",
			minimum: 10,
			maximum: 10.001,
			source: { kind: "raw_dmx", universe: 1, address: 512 },
			graph: { timeWindowSeconds: 3_600, lineLowColor: "#32a9c5" },
		});
	});

	it("interpolates configured range colours", () => {
		expect(mixedColor("#000000", "#ffffff", 0.5)).toBe("#808080");
	});
});
