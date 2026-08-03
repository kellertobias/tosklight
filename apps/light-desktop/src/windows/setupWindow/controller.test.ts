import { describe, expect, it } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import {
	configurationFieldsForSection,
	mergeConfigurationFields,
} from "./controller";

function configuration(
	overrides: Partial<DeskConfiguration> = {},
): DeskConfiguration {
	return {
		frame_rate_hz: 44,
		output_bind_ip: "0.0.0.0",
		autosave_interval_seconds: 30,
		backup_retention: 20,
		command_line_at_uses_programmer_fade: true,
		preload_programmer_changes: true,
		preload_physical_playback_actions: true,
		preload_virtual_playback_actions: true,
		patch_preview_highlight_dmx: false,
		...overrides,
	} as DeskConfiguration;
}

describe("Desk Setup page-scoped configuration saves", () => {
	it("assigns each editable Preferences page only its owned configuration fields", () => {
		expect(configurationFieldsForSection("preferences-defaults")).toEqual([]);
		expect(configurationFieldsForSection("preferences-attributes")).toEqual([]);
		expect(configurationFieldsForSection("preferences-highlight")).toEqual([
			"highlight_look",
			"patch_preview_highlight_dmx",
		]);
		expect(configurationFieldsForSection("preferences-others")).toEqual([
			"command_line_at_uses_programmer_fade",
			"preload_programmer_changes",
			"preload_physical_playback_actions",
			"preload_virtual_playback_actions",
		]);
	});

	it("does not absorb an unsaved field from another Preferences page", () => {
		const saved = configuration({
			command_line_at_uses_programmer_fade: true,
			patch_preview_highlight_dmx: false,
		});
		const draft = configuration({
			command_line_at_uses_programmer_fade: false,
			patch_preview_highlight_dmx: true,
		});

		expect(
			mergeConfigurationFields(
				saved,
				draft,
				configurationFieldsForSection("preferences-highlight"),
			),
		).toMatchObject({
			command_line_at_uses_programmer_fade: true,
			patch_preview_highlight_dmx: true,
		});
	});
});
