import { useCallback, useEffect, useRef, useState } from "react";
import { configuredServerUrl } from "../../api/client/serverLocation";
import type {
	AttributeConfiguration,
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
} from "../../api/generated/light-wire";
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

export function useSetupWindowController() {
	const connection = useDeskConnection();
	const configurationActions = useConfigurationActions();
	const attributeActions = useAttributeConfigurationActions();
	const configuration = useDeskConfiguration();
	const programmingUpdate = useProgrammingUpdate();
	const [section, setSection] = useState(0);
	const [draft, setDraft] = useState<DeskConfiguration | null>(configuration);
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
	const [attributeConfiguration, setAttributeConfiguration] =
		useState<AttributeConfigurationSnapshot | null>(null);
	const [attributeConfigurationError, setAttributeConfigurationError] =
		useState<string | null>(null);
	const [restartRequired, setRestartRequired] = useState(false);
	const [serverUrl, setServerUrl] = useState(configuredServerUrl());
	const [fixtureLibraryOpen, setFixtureLibraryOpen] = useState(false);
	const [deskLockSettingsOpen, setDeskLockSettingsOpen] = useState(false);
	const [screenCanUndo, setScreenCanUndo] = useState(false);
	const screenUndo = useRef<(() => void) | null>(null);
	const draftRevision = useRef(0);
	const draftDirty = useRef(false);
	const pendingSave = useRef<{
		revision: number;
		configuration: DeskConfiguration;
	} | null>(null);
	const savedAttributeConfiguration =
		useRef<AttributeConfigurationSnapshot | null>(null);

	useEffect(() => {
		const pending = pendingSave.current;
		if (
			pending &&
			JSON.stringify(pending.configuration) === JSON.stringify(configuration)
		) {
			pendingSave.current = null;
			if (draftRevision.current === pending.revision) {
				draftDirty.current = false;
				setDraft(configuration);
			}
			return;
		}
		if (!draftDirty.current) setDraft(configuration);
	}, [configuration]);

	useEffect(() => {
		if (section !== 2) return;
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

	useEffect(() => {
		if (section !== 2) return;
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
		draftRevision.current += 1;
		draftDirty.current = true;
		setDraft(next);
	};
	const save = async () => {
		if (!draft) return;
		pendingSave.current = {
			revision: draftRevision.current,
			configuration: draft,
		};
		const [requiresRestart, updateSaved, attributesSaved] = await Promise.all([
			configurationActions?.saveConfiguration(draft) ?? Promise.resolve(false),
			section === 2 && programmerSettingsLoaded
				? saveUpdateSettings(programmingUpdate, updateSettings)
				: Promise.resolve(true),
			section === 2
				? saveAttributeConfiguration(
						attributeActions,
						savedAttributeConfiguration.current,
						attributeConfiguration,
					)
				: Promise.resolve(attributeConfiguration),
		]);
		if (section === 2) saveRecordSettings(recordSettings);
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
