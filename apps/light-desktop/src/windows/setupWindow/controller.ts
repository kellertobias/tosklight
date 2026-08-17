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
import type {
	AttributeSettingsTab,
	DefaultsSettingsTab,
	NetworkSettingsTab,
	OutputsSettingsTab,
	SetupSection,
} from "./SetupChrome";

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

function useAttributeConfigurationSetup(
	actions: ReturnType<typeof useAttributeConfigurationActions>,
	section: SetupSection,
) {
	const [configuration, setConfiguration] =
		useState<AttributeConfigurationSnapshot | null>(null);
	const [error, setError] = useState<string | null>(null);
	const saved = useRef<AttributeConfigurationSnapshot | null>(null);
	const current = useRef<AttributeConfigurationSnapshot | null>(null);
	const saveQueue = useRef(Promise.resolve());
	useEffect(() => {
		if (section !== "preferences-attributes") return;
		let active = true;
		setError(null);
		void actions
			?.load()
			.then((snapshot) => {
				if (!active) return;
				saved.current = snapshot;
				current.current = snapshot;
				setConfiguration(snapshot);
				setError(snapshot.validation_error);
			})
			.catch((reason) => {
				if (!active) return;
				setConfiguration(null);
				setError(errorMessage(reason));
			});
		if (!actions) setError("Show attribute configuration is unavailable.");
		return () => {
			active = false;
		};
	}, [actions, section]);
	const editConfiguration = useCallback(
		(nextConfiguration: AttributeConfiguration) => {
			const snapshot = current.current;
			if (!snapshot) return;
			const next = { ...snapshot, configuration: nextConfiguration };
			current.current = next;
			setConfiguration(next);
			saveQueue.current = saveQueue.current.then(async () => {
				const savedSnapshot = saved.current;
				const latest = current.current;
				if (!savedSnapshot || !latest) return;
				const result = await saveAttributeConfiguration(
					actions,
					savedSnapshot,
					latest,
				);
				if (!result) {
					setError("Attribute configuration was not saved.");
					return;
				}
				saved.current = result;
				current.current = { ...result, configuration: latest.configuration };
				setConfiguration(current.current);
				setError(result.validation_error);
			});
		},
		[actions],
	);
	return { configuration, editConfiguration, error };
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
	} = useProgrammerSetupSettings(programmingUpdate, section);
	const {
		configuration: attributeConfiguration,
		editConfiguration: editAttributeConfiguration,
		error: attributeConfigurationError,
	} = useAttributeConfigurationSetup(attributeActions, section);
	const [restartRequired, setRestartRequired] = useState(false);
	const [serverUrl, setServerUrl] = useState(configuredServerUrl());
	const [fixtureLibraryOpen, setFixtureLibraryOpen] = useState(false);
	const [deskLockSettingsOpen, setDeskLockSettingsOpen] = useState(false);
	const [encoderPlacementOpen, setEncoderPlacementOpen] = useState(false);
	const [screenCanUndo, setScreenCanUndo] = useState(false);
	const [attributeTab, setAttributeTab] =
		useState<AttributeSettingsTab>("encoder-groups");
	const [networkTab, setNetworkTab] =
		useState<NetworkSettingsTab>("control-server");
	const [defaultsTab, setDefaultsTab] =
		useState<DefaultsSettingsTab>("record-update");
	const [outputsTab, setOutputsTab] = useState<OutputsSettingsTab>("engine");
	const screenUndo = useRef<(() => void) | null>(null);
	const deskSaveQueue = useRef(Promise.resolve());
	const lastQueuedConfiguration = useRef<string | null>(null);

	const editDraft = (next: DeskConfiguration) => {
		if (draft)
			for (const field of CONFIGURATION_FIELDS)
				if (!sameValue(draft[field], next[field]))
					dirtyFields.current.add(field);
		setDraft(next);
	};
	useEffect(() => {
		if (!configuration || !configurationActions || !draft) return;
		const fields = CONFIGURATION_FIELDS.filter((field) =>
			dirtyFields.current.has(field),
		);
		if (!fields.length) return;
		const next = mergeConfigurationFields(configuration, draft, fields);
		const fingerprint = JSON.stringify(next);
		if (lastQueuedConfiguration.current === fingerprint) return;
		lastQueuedConfiguration.current = fingerprint;
		pendingSave.current = { fields, configuration: next };
		deskSaveQueue.current = deskSaveQueue.current.then(async () => {
			const requiresRestart =
				await configurationActions.saveConfiguration(next);
			setRestartRequired((current) => current || requiresRestart);
		});
	}, [configuration, configurationActions, draft, dirtyFields, pendingSave]);
	const updateScreenUndoAvailability = useCallback(
		(available: boolean) => setScreenCanUndo(available),
		[],
	);

	return {
		attributeTab,
		setAttributeTab,
		defaultsTab,
		setDefaultsTab,
		deskLockSettingsOpen,
		encoderPlacementOpen,
		draft,
		editDraft,
		attributeConfiguration,
		attributeConfigurationError,
		editAttributeConfiguration,
		fixtureLibraryOpen,
		programmerSettingsError,
		programmerSettingsLoaded,
		recordSettings,
		restartRequired,
		networkTab,
		outputsTab,
		screenCanUndo,
		screenUndo,
		section,
		applyServerUrl: (url: string) => connection?.setServerUrl(url),
		serverUrl,
		setDeskLockSettingsOpen,
		setEncoderPlacementOpen,
		setFixtureLibraryOpen,
		setRecordSettings,
		setNetworkTab,
		setOutputsTab,
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
	const loadedOnce = useRef(false);
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
	const updateSaveQueue = useRef(Promise.resolve());
	useEffect(() => {
		if (section !== "preferences-defaults" || loadedOnce.current) return;
		loadedOnce.current = true;
		setProgrammerSettingsLoaded(false);
		setRecordSettings(loadRecordSettings());
		setProgrammerSettingsError(null);
		void programmingUpdate
			?.loadSettings()
			.then((settings) => {
				setUpdateSettings(settings ?? defaultUpdateSettings);
				setProgrammerSettingsLoaded(true);
				if (!settings)
					setProgrammerSettingsError(
						"Update defaults could not be loaded; deterministic defaults are shown.",
					);
			})
			.catch((reason) => {
				setUpdateSettings(defaultUpdateSettings);
				setProgrammerSettingsLoaded(true);
				setProgrammerSettingsError(errorMessage(reason));
			});
		if (!programmingUpdate) {
			setUpdateSettings(defaultUpdateSettings);
			setProgrammerSettingsLoaded(true);
			setProgrammerSettingsError("Update defaults are unavailable.");
		}
	}, [programmingUpdate, section]);
	const editRecordSettings = useCallback((settings: RecordSettings) => {
		setRecordSettings(settings);
		saveRecordSettings(settings);
	}, []);
	const editUpdateSettings = useCallback(
		(settings: UpdateSettings) => {
			setUpdateSettings(settings);
			if (!programmerSettingsLoaded) return;
			updateSaveQueue.current = updateSaveQueue.current.then(async () => {
				const saved = await saveUpdateSettings(programmingUpdate, settings);
				setProgrammerSettingsError(
					saved ? null : "Update defaults were not saved.",
				);
			});
		},
		[programmerSettingsLoaded, programmingUpdate],
	);
	return {
		recordSettings,
		setRecordSettings: editRecordSettings,
		updateSettings,
		setUpdateSettings: editUpdateSettings,
		programmerSettingsLoaded,
		programmerSettingsError,
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
	internal_audio_library_roots: true,
	internal_audio_output_devices: true,
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
			return ["autosave_interval_seconds", "internal_audio_library_roots"];
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
				"internal_audio_output_devices",
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
