import { useCallback, useEffect, useRef, useState } from "react";
import type {
	AttributeConfiguration,
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
} from "../../api/client/attributeConfiguration";
import { configuredServerUrl } from "../../api/client/serverLocation";
import type { DeskConfiguration, UpdateSettings } from "../../api/types";
import { defaultUpdateSettings } from "../../components/control/updateWorkflow";
import {
	loadRecordSettings,
	type RecordSettings,
	saveRecordSettings,
} from "../../components/setup/ProgrammerDefaults";
import { useAttributeConfigurationActions } from "../../features/attributeConfiguration/AttributeConfigurationActions";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import { useDeskConfiguration } from "../../features/configuration/ConfigurationState";
import { useDeskConnection } from "../../features/deskConnection/DeskConnectionContext";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import type { AttributeSettingsTab, SetupSection } from "./SetupChrome";

function useDeskConfigurationDraft(configuration: DeskConfiguration | null) {
	const [draft, setDraft] = useState<DeskConfiguration | null>(configuration);
	const pendingSave = useRef<{
		fields: DeskConfigurationField[];
		configuration: DeskConfiguration;
	} | null>(null);
	const dirtyFields = useRef(new Set<DeskConfigurationField>());
	useEffect(() => {
		if (!configuration) return;
		setDraft((current) => {
			if (!current) return configuration;
			const pending = pendingSave.current;
			const confirmed =
				pending && sameConfiguration(pending.configuration, configuration)
					? pending
					: null;
			if (confirmed) {
				pendingSave.current = null;
				for (const field of confirmed.fields)
					if (sameValue(current[field], confirmed.configuration[field]))
						dirtyFields.current.delete(field);
			}
			const merged = { ...current };
			for (const field of CONFIGURATION_FIELDS)
				if (!dirtyFields.current.has(field))
					assignField(merged, configuration, field);
			return merged;
		});
	}, [configuration]);
	return { draft, setDraft, pendingSave, dirtyFields };
}

export function useSetupWindowController() {
	const connection = useDeskConnection();
	const configurationActions = useConfigurationActions();
	const attributeActions = useAttributeConfigurationActions();
	const configuration = useDeskConfiguration();
	const programmingUpdate = useProgrammingUpdate();
	const [section, setSection] = useState<SetupSection>("shows");
	const { draft, setDraft, pendingSave, dirtyFields } =
		useDeskConfigurationDraft(configuration);
	const {
		recordSettings,
		setRecordSettings,
		updateSettings,
		setUpdateSettings,
		programmerSettingsLoaded,
		programmerSettingsError,
		setProgrammerSettingsError,
	} = useProgrammerSetupSettings(programmingUpdate, section);
	const [attributeConfiguration, setAttributeConfiguration] =
		useState<AttributeConfigurationSnapshot | null>(null);
	const [attributeConfigurationError, setAttributeConfigurationError] =
		useState<string | null>(null);
	const [restartRequired, setRestartRequired] = useState(false);
	const [serverUrl, setServerUrl] = useState(configuredServerUrl());
	const [fixtureLibraryOpen, setFixtureLibraryOpen] = useState(false);
	const [deskLockSettingsOpen, setDeskLockSettingsOpen] = useState(false);
	const [screenCanUndo, setScreenCanUndo] = useState(false);
	const [attributeTab, setAttributeTab] =
		useState<AttributeSettingsTab>("encoder-groups");
	const screenUndo = useRef<(() => void) | null>(null);
	const savedAttributeConfiguration =
		useRef<AttributeConfigurationSnapshot | null>(null);

	useEffect(() => {
		if (section !== "preferences-attributes") return;
		let active = true;
		setAttributeConfigurationError(null);
		void attributeActions
			?.load()
			.then((snapshot) => {
				if (!active) return;
				savedAttributeConfiguration.current = snapshot;
				setAttributeConfiguration(snapshot);
				setAttributeConfigurationError(snapshot.validation_error);
			})
			.catch((reason) => {
				if (!active) return;
				setAttributeConfiguration(null);
				setAttributeConfigurationError(errorMessage(reason));
			});
		if (!attributeActions)
			setAttributeConfigurationError(
				"Show attribute configuration is unavailable.",
			);
		return () => {
			active = false;
		};
	}, [attributeActions, section]);

	const editDraft = (next: DeskConfiguration) => {
		if (draft)
			for (const field of CONFIGURATION_FIELDS)
				if (!sameValue(draft[field], next[field]))
					dirtyFields.current.add(field);
		setDraft(next);
	};
	const save = async () => {
		if (!draft) return;
		const deskFields = configurationFieldsForSection(section).filter((field) =>
			dirtyFields.current.has(field),
		);
		const deskConfiguration = configurationForSave(
			configuration,
			draft,
			deskFields,
		);
		pendingSave.current = deskConfiguration
			? { fields: deskFields, configuration: deskConfiguration }
			: null;
		const [requiresRestart, updateSaved, attributesSaved] = await Promise.all([
			deskConfiguration
				? (configurationActions?.saveConfiguration(deskConfiguration) ??
					Promise.resolve(false))
				: Promise.resolve(false),
			section === "preferences-defaults" && programmerSettingsLoaded
				? saveUpdateSettings(programmingUpdate, updateSettings)
				: Promise.resolve(true),
			section === "preferences-attributes"
				? saveAttributeConfiguration(
						attributeActions,
						savedAttributeConfiguration.current,
						attributeConfiguration,
					)
				: Promise.resolve(attributeConfiguration),
		]);
		if (section === "preferences-defaults") saveRecordSettings(recordSettings);
		if (attributesSaved) {
			savedAttributeConfiguration.current = attributesSaved;
			setAttributeConfiguration(attributesSaved);
		}
		setRestartRequired(requiresRestart);
		setProgrammerSettingsError(
			updateSaved ? null : "Update defaults were not saved.",
		);
		setAttributeConfigurationError(
			attributesSaved
				? attributesSaved.validation_error
				: "Attribute configuration was not saved.",
		);
	};
	const updateScreenUndoAvailability = useCallback(
		(available: boolean) => setScreenCanUndo(available),
		[],
	);

	return {
		attributeTab,
		setAttributeTab,
		deskLockSettingsOpen,
		draft,
		editDraft,
		attributeConfiguration,
		attributeConfigurationError,
		editAttributeConfiguration: (configuration: AttributeConfiguration) =>
			setAttributeConfiguration((current) =>
				current ? { ...current, configuration } : current,
			),
		fixtureLibraryOpen,
		programmerSettingsError,
		programmerSettingsLoaded,
		recordSettings,
		restartRequired,
		save,
		screenCanUndo,
		screenUndo,
		section,
		applyServerUrl: (url: string) => connection?.setServerUrl(url),
		serverUrl,
		setDeskLockSettingsOpen,
		setFixtureLibraryOpen,
		setRecordSettings,
		setSection,
		setServerUrl,
		setUpdateSettings,
		updateSettings,
		updateScreenUndoAvailability,
	};
}

export type SetupWindowController = ReturnType<typeof useSetupWindowController>;

function useProgrammerSetupSettings(
	programmingUpdate: ReturnType<typeof useProgrammingUpdate>,
	section: SetupSection,
) {
	const [recordSettings, setRecordSettings] =
		useState<RecordSettings>(loadRecordSettings);
	const [updateSettings, setUpdateSettings] = useState<UpdateSettings>(
		defaultUpdateSettings,
	);
	const [programmerSettingsLoaded, setProgrammerSettingsLoaded] =
		useState(false);
	const [programmerSettingsError, setProgrammerSettingsError] = useState<
		string | null
	>(null);
	useEffect(() => {
		if (section !== "preferences-defaults") return;
		let active = true;
		setProgrammerSettingsLoaded(false);
		setRecordSettings(loadRecordSettings());
		setProgrammerSettingsError(null);
		void programmingUpdate
			?.loadSettings()
			.then((settings) => {
				if (!active) return;
				setUpdateSettings(settings ?? defaultUpdateSettings);
				setProgrammerSettingsLoaded(true);
				if (!settings)
					setProgrammerSettingsError(
						"Update defaults could not be loaded; deterministic defaults are shown.",
					);
			})
			.catch((reason) => {
				if (!active) return;
				setUpdateSettings(defaultUpdateSettings);
				setProgrammerSettingsLoaded(true);
				setProgrammerSettingsError(errorMessage(reason));
			});
		if (!programmingUpdate) {
			setUpdateSettings(defaultUpdateSettings);
			setProgrammerSettingsLoaded(true);
			setProgrammerSettingsError("Update defaults are unavailable.");
		}
		return () => {
			active = false;
		};
	}, [programmingUpdate, section]);
	return {
		recordSettings,
		setRecordSettings,
		updateSettings,
		setUpdateSettings,
		programmerSettingsLoaded,
		programmerSettingsError,
		setProgrammerSettingsError,
	};
}

type DeskConfigurationField = keyof DeskConfiguration;

const CONFIGURATION_FIELDS = Object.keys({
	frame_rate_hz: true,
	output_bind_ip: true,
	osc_bind: true,
	art_timecode_bind: true,
	timecode_source: true,
	timecode_frame_rate: true,
	timecode_external_loss_policy: true,
	timecode_external_loss_timeout_millis: true,
	osc_timecode: true,
	timecode_audio_output_device: true,
	timecode_audio_latency_trim_micros_by_output: true,
	backup_retention: true,
	autosave_interval_seconds: true,
	speed_groups_bpm: true,
	programmer_fade_millis: true,
	command_line_at_uses_programmer_fade: true,
	sequence_master_fade_millis: true,
	cuelist_auto_off_at_zero_default: true,
	cuelist_auto_off_flash_release_default: true,
	start_after_first_recording: true,
	preload_programmer_changes: true,
	preload_physical_playback_actions: true,
	preload_virtual_playback_actions: true,
	patch_preview_highlight_dmx: true,
	highlight_look: true,
	highlight_look_feedback: true,
	matter_enabled: true,
	pool_presentation: true,
	update_settings_by_desk: true,
	file_manager_system_picker_fallback: true,
	file_manager_roots: true,
} satisfies Record<DeskConfigurationField, true>) as DeskConfigurationField[];

export function configurationFieldsForSection(
	section: SetupSection,
): DeskConfigurationField[] {
	switch (section) {
		case "shows":
			return ["autosave_interval_seconds"];
		case "outputs":
			return ["frame_rate_hz", "output_bind_ip", "backup_retention"];
		case "timecode":
			return [
				"timecode_source",
				"timecode_frame_rate",
				"timecode_external_loss_policy",
				"timecode_external_loss_timeout_millis",
				"art_timecode_bind",
				"osc_timecode",
				"timecode_audio_output_device",
				"timecode_audio_latency_trim_micros_by_output",
			];
		case "preferences-highlight":
			return ["highlight_look", "patch_preview_highlight_dmx"];
		case "preferences-others":
			return [
				"command_line_at_uses_programmer_fade",
				"preload_programmer_changes",
				"preload_physical_playback_actions",
				"preload_virtual_playback_actions",
			];
		case "preferences-defaults":
			return [
				"cuelist_auto_off_at_zero_default",
				"cuelist_auto_off_flash_release_default",
				"start_after_first_recording",
			];
		default:
			return [];
	}
}

export function mergeConfigurationFields(
	base: DeskConfiguration,
	draft: DeskConfiguration,
	fields: readonly DeskConfigurationField[],
) {
	const merged = { ...base };
	for (const field of fields) assignField(merged, draft, field);
	return merged;
}

export function configurationForSave(
	base: DeskConfiguration | null,
	draft: DeskConfiguration,
	fields: readonly DeskConfigurationField[],
) {
	if (!base || fields.length === 0) return null;
	return mergeConfigurationFields(base, draft, fields);
}

function assignField(
	target: DeskConfiguration,
	source: DeskConfiguration,
	field: DeskConfigurationField,
) {
	Object.assign(target, { [field]: source[field] });
}

function sameConfiguration(left: DeskConfiguration, right: DeskConfiguration) {
	return sameValue(left, right);
}

function sameValue(left: unknown, right: unknown) {
	return JSON.stringify(left) === JSON.stringify(right);
}

function saveUpdateSettings(
	update: ReturnType<typeof useProgrammingUpdate>,
	settings: UpdateSettings,
) {
	if (!update) return Promise.resolve(false);
	return update
		.saveSettings(settings)
		.then(Boolean)
		.catch(() => false);
}

async function saveAttributeConfiguration(
	actions: ReturnType<typeof useAttributeConfigurationActions>,
	saved: AttributeConfigurationSnapshot | null,
	current: AttributeConfigurationSnapshot | null,
) {
	if (!actions || !saved || !current) return null;
	const patch = attributeConfigurationPatch(
		saved.configuration,
		current.configuration,
	);
	if (!Object.keys(patch).length) return current;
	try {
		return await actions.update(saved, patch);
	} catch {
		return null;
	}
}

function attributeConfigurationPatch(
	saved: AttributeConfiguration,
	current: AttributeConfiguration,
): AttributeConfigurationPatch {
	const patch: AttributeConfigurationPatch = {};
	if (
		JSON.stringify(saved.custom_attributes) !==
		JSON.stringify(current.custom_attributes)
	)
		patch.custom_attributes = current.custom_attributes;
	if (JSON.stringify(saved.placements) !== JSON.stringify(current.placements))
		patch.placements = current.placements;
	if (
		JSON.stringify(saved.activation_groups) !==
		JSON.stringify(current.activation_groups)
	)
		patch.activation_groups = current.activation_groups;
	return patch;
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
