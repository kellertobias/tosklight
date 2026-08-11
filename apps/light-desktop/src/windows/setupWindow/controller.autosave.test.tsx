// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AttributeConfigurationSnapshot } from "../../api/client/attributeConfiguration";
import type { DeskConfiguration, UpdateSettings } from "../../api/types";
import { defaultUpdateSettings } from "../../components/control/updateWorkflow";
import { useSetupWindowController } from "./controller";

const setup = vi.hoisted(() => ({
	configuration: null as DeskConfiguration | null,
	updateSettings: null as UpdateSettings | null,
	attributes: null as AttributeConfigurationSnapshot | null,
	saveConfiguration: vi.fn(),
	loadUpdateSettings: vi.fn(),
	saveUpdateSettings: vi.fn(),
	loadAttributes: vi.fn(),
	updateAttributes: vi.fn(),
}));

const storedValues = new Map<string, string>();

vi.mock("../../features/configuration/ConfigurationState", () => ({
	useDeskConfiguration: () => setup.configuration,
}));
vi.mock("../../features/configuration/ConfigurationActionsProvider", () => ({
	useConfigurationActions: () => ({
		saveConfiguration: setup.saveConfiguration,
	}),
}));
vi.mock("../../features/programmingUpdate/ProgrammingUpdateProvider", () => ({
	useProgrammingUpdate: () => ({
		loadSettings: setup.loadUpdateSettings,
		saveSettings: setup.saveUpdateSettings,
	}),
}));
vi.mock(
	"../../features/attributeConfiguration/AttributeConfigurationActions",
	() => ({
		useAttributeConfigurationActions: () => ({
			canWrite: true,
			load: setup.loadAttributes,
			update: setup.updateAttributes,
		}),
	}),
);
vi.mock("../../features/deskConnection/DeskConnectionContext", () => ({
	useDeskConnection: () => null,
}));

beforeEach(() => {
	storedValues.clear();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => storedValues.get(key) ?? null,
		setItem: (key: string, value: string) => storedValues.set(key, value),
		removeItem: (key: string) => storedValues.delete(key),
		clear: () => storedValues.clear(),
	});
	setup.configuration = configuration();
	setup.updateSettings = { ...defaultUpdateSettings };
	setup.attributes = attributeSnapshot();
	setup.saveConfiguration.mockReset().mockImplementation(async (next) => {
		setup.configuration = next;
		return false;
	});
	setup.loadUpdateSettings
		.mockReset()
		.mockImplementation(async () => setup.updateSettings);
	setup.saveUpdateSettings.mockReset().mockImplementation(async (next) => {
		setup.updateSettings = next;
		return next;
	});
	setup.loadAttributes
		.mockReset()
		.mockImplementation(async () => setup.attributes);
	setup.updateAttributes
		.mockReset()
		.mockImplementation(async (_snapshot, patch) => {
			const attributes = setup.attributes;
			if (!attributes) throw new Error("Attribute configuration is missing");
			setup.attributes = {
				...attributes,
				show_revision: attributes.show_revision + 1,
				object_revision: attributes.object_revision + 1,
				configuration: { ...attributes.configuration, ...patch },
			};
			return setup.attributes;
		});
});

describe("Desk Setup autosave", () => {
	it("persists a desk field immediately and reloads it without a save action", async () => {
		const first = renderHook(useSetupWindowController);
		const draft = first.result.current.draft;
		if (!draft) throw new Error("Desk configuration draft is missing");
		act(() => {
			first.result.current.editDraft({
				...draft,
				autosave_interval_seconds: 90,
			});
		});
		await waitFor(() => expect(setup.saveConfiguration).toHaveBeenCalledOnce());
		expect(setup.saveConfiguration.mock.calls[0][0]).toMatchObject({
			autosave_interval_seconds: 90,
		});
		first.unmount();

		const reopened = renderHook(useSetupWindowController);
		expect(reopened.result.current.draft?.autosave_interval_seconds).toBe(90);
	});

	it("persists Record and Update defaults on change and reloads both", async () => {
		const first = renderHook(useSetupWindowController);
		act(() => first.result.current.setSection("preferences-defaults"));
		await waitFor(() =>
			expect(first.result.current.programmerSettingsLoaded).toBe(true),
		);
		act(() => {
			first.result.current.setRecordSettings({
				mode: "overwrite",
				cueOnly: true,
				mergeActiveCue: true,
			});
			first.result.current.setUpdateSettings({
				...defaultUpdateSettings,
				show_update_modal_on_touch: false,
			});
		});
		await waitFor(() =>
			expect(setup.saveUpdateSettings).toHaveBeenCalledOnce(),
		);
		first.unmount();

		const reopened = renderHook(useSetupWindowController);
		act(() => reopened.result.current.setSection("preferences-defaults"));
		await waitFor(() =>
			expect(reopened.result.current.programmerSettingsLoaded).toBe(true),
		);
		expect(reopened.result.current.recordSettings).toEqual({
			mode: "overwrite",
			cueOnly: true,
			mergeActiveCue: true,
		});
		expect(
			reopened.result.current.updateSettings.show_update_modal_on_touch,
		).toBe(false);
	});

	it("serializes an Attribute edit through its revisioned owner", async () => {
		const first = renderHook(useSetupWindowController);
		act(() => first.result.current.setSection("preferences-attributes"));
		await waitFor(() =>
			expect(first.result.current.attributeConfiguration).not.toBeNull(),
		);
		const attributes = first.result.current.attributeConfiguration;
		if (!attributes) throw new Error("Attribute configuration is missing");
		act(() =>
			first.result.current.editAttributeConfiguration({
				...attributes.configuration,
				placements: [
					{
						attribute: "intensity",
						encoder_group: "intensity",
						encoder_page: 1,
						encoder_slot: 1,
					},
				],
			}),
		);
		await waitFor(() => expect(setup.updateAttributes).toHaveBeenCalledOnce());
		expect(setup.updateAttributes.mock.calls[0][1]).toEqual({
			placements: [
				{
					attribute: "intensity",
					encoder_group: "intensity",
					encoder_page: 1,
					encoder_slot: 1,
				},
			],
		});
	});
});

function configuration(): DeskConfiguration {
	return {
		frame_rate_hz: 44,
		output_bind_ip: "0.0.0.0",
		autosave_interval_seconds: 30,
		backup_retention: 20,
		command_line_at_uses_programmer_fade: false,
		cuelist_auto_off_at_zero_default: false,
		cuelist_auto_off_flash_release_default: false,
		start_after_first_recording: false,
		preload_programmer_changes: true,
		preload_physical_playback_actions: false,
		preload_virtual_playback_actions: true,
		patch_preview_highlight_dmx: false,
	} as DeskConfiguration;
}

function attributeSnapshot(): AttributeConfigurationSnapshot {
	const configuration = {
		version: 1,
		custom_attributes: [],
		placements: [],
		activation_groups: [],
	};
	return {
		show_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
		show_revision: 1,
		object_revision: 1,
		configuration,
		recommended_configuration: configuration,
		descriptors: [],
		validation_error: null,
	};
}
