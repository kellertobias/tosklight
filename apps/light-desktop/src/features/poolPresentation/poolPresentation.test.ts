import { describe, expect, it } from "vitest";
import { migrateLegacyPresetPresentations } from "./PoolPresentationLegacyMigration";
import {
	defaultPoolPresentation,
	poolItemKey,
	poolSurfaceKey,
	resetAllPoolColors,
	resetPoolColor,
	resolveConfiguredPoolPresentation,
} from "./poolPresentation";

describe("desk-owned pool presentation", () => {
	it("qualifies surface and item preferences by show", () => {
		expect(poolSurfaceKey("show-a", "group", "pane-1")).not.toBe(
			poolSurfaceKey("show-b", "group", "pane-1"),
		);
		expect(poolItemKey("show-a", "preset", "2.1")).not.toBe(
			poolItemKey("show-b", "preset", "2.1"),
		);
	});

	it("switches between type defaults and explicit individual colors", () => {
		const configuration = defaultPoolPresentation();
		const surfaceKey = poolSurfaceKey("show-a", "group");
		expect(
			resolveConfiguredPoolPresentation(configuration, {
				showId: "show-a",
				surfaceKey,
				objectType: "group",
				itemColorKey: "1",
				itemColor: "#123456",
			}).color,
		).toBe("#d8ad55");
		configuration.modes[surfaceKey] = "individual";
		expect(
			resolveConfiguredPoolPresentation(configuration, {
				showId: "show-a",
				surfaceKey,
				objectType: "group",
				itemColorKey: "1",
				itemColor: "#123456",
			}).color,
		).toBe("#123456");
	});

	it("migrates legacy Preset customizations once without replacing desk values", () => {
		const configuration = defaultPoolPresentation();
		configuration.items[poolItemKey("show-a", "preset", "2.1")] = {
			color: "#111111",
		};
		const migrated = migrateLegacyPresetPresentations(configuration, "show-a", {
			"2.1": { color: "#222222" },
			"2.2": { title: "Ocean", color: "#2255aa" },
		});
		expect(migrated.items[poolItemKey("show-a", "preset", "2.1")].color).toBe(
			"#111111",
		);
		expect(migrated.items[poolItemKey("show-a", "preset", "2.2")]).toEqual({
			title: "Ocean",
			color: "#2255aa",
		});
	});

	it("resets one color without changing other palette or item settings", () => {
		const configuration = defaultPoolPresentation();
		configuration.palette.group = "#111111";
		configuration.palette.preset.position = "#222222";
		configuration.palette.preset.beam = "#333333";
		configuration.items[poolItemKey("show-a", "group", "1")] = {
			color: "#444444",
		};

		const reset = resetPoolColor(configuration, "preset", "position");
		expect(reset.palette.preset.position).toBe(
			defaultPoolPresentation().palette.preset.position,
		);
		expect(reset.palette.preset.beam).toBe("#333333");
		expect(reset.palette.group).toBe("#111111");
		expect(reset.items).toEqual(configuration.items);
	});

	it("resets the whole palette without deleting modes or item colors", () => {
		const configuration = defaultPoolPresentation();
		configuration.palette.group = "#111111";
		configuration.modes["show:show-a:builtin:group"] = "individual";
		configuration.items[poolItemKey("show-a", "group", "1")] = {
			color: "#444444",
		};

		const reset = resetAllPoolColors(configuration);
		expect(reset.palette).toEqual(defaultPoolPresentation().palette);
		expect(reset.modes).toEqual(configuration.modes);
		expect(reset.items).toEqual(configuration.items);
	});
});
