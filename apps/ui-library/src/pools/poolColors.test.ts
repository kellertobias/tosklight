import { describe, expect, it } from "vitest";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	INDIVIDUAL_POOL_COLOR_FALLBACK,
	type PoolObjectType,
	resolvePoolPresentation,
} from "./poolColors";

describe("resolvePoolPresentation", () => {
	it.each([
		["group", DEFAULT_POOL_COLOR_PALETTE.group],
		["macro", DEFAULT_POOL_COLOR_PALETTE.macro],
		["dynamic", DEFAULT_POOL_COLOR_PALETTE.dynamic],
		["cuelist", DEFAULT_POOL_COLOR_PALETTE.cuelist],
		["sequence", DEFAULT_POOL_COLOR_PALETTE.sequence],
	] satisfies [
		PoolObjectType,
		string,
	][])("uses the configured %s type color", (objectType, color) => {
		expect(resolvePoolPresentation({ objectType, mode: "type" }).color).toBe(
			color,
		);
	});

	it("resolves every Preset family independently", () => {
		const palette = {
			...DEFAULT_POOL_COLOR_PALETTE,
			preset: {
				mixed: "#111111",
				intensity: "#222222",
				color: "#333333",
				position: "#444444",
				beam: "#555555",
			},
		};
		expect(
			resolvePoolPresentation({
				objectType: "preset",
				presetFamily: "position",
				mode: "type",
				palette,
			}).color,
		).toBe("#444444");
	});

	it("uses only explicit item colors in individual mode", () => {
		expect(
			resolvePoolPresentation({
				objectType: "group",
				mode: "individual",
				itemColor: "#123456",
			}).color,
		).toBe("#123456");
		expect(
			resolvePoolPresentation({
				objectType: "group",
				mode: "individual",
			}).color,
		).toBe(INDIVIDUAL_POOL_COLOR_FALLBACK);
	});

	it("keeps workflow and accessibility states as non-color classes", () => {
		const presentation = resolvePoolPresentation({
			objectType: "cuelist",
			mode: "type",
			states: [
				"selected",
				"focused",
				"record-target",
				"update-target",
				"copy-target",
				"move-target",
				"delete-target",
				"disabled",
				"empty",
				"selected",
			],
		});
		expect(presentation.states).toEqual([
			"selected",
			"focused",
			"record-target",
			"update-target",
			"copy-target",
			"move-target",
			"delete-target",
			"disabled",
			"empty",
		]);
		expect(presentation.className).toContain("pool-type-cuelist");
		expect(presentation.className).toContain("record-target");
		expect(presentation.style).not.toHaveProperty("--pool-card-color");
	});
});
