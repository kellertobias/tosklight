import { useCallback, useEffect, useRef, useState } from "react";
import { configuredServerUrl } from "../../api/LightApiClient";
import { useServer } from "../../api/ServerContext";
import type { DeskConfiguration, UpdateSettings } from "../../api/types";
import { defaultUpdateSettings } from "../../components/control/updateWorkflow";
import {
	loadRecordSettings,
	type RecordSettings,
	saveRecordSettings,
} from "../../components/setup/ProgrammerDefaults";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";

export function useSetupWindowController() {
	const server = useServer();
	const programmingUpdate = useProgrammingUpdate();
	const [section, setSection] = useState(0);
	const [draft, setDraft] = useState<DeskConfiguration | null>(
		server.configuration,
	);
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

	useEffect(() => {
		const pending = pendingSave.current;
		if (
			pending &&
			JSON.stringify(pending.configuration) ===
				JSON.stringify(server.configuration)
		) {
			pendingSave.current = null;
			if (draftRevision.current === pending.revision) {
				draftDirty.current = false;
				setDraft(server.configuration);
			}
			return;
		}
		if (!draftDirty.current) setDraft(server.configuration);
	}, [server.configuration]);

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
		const [requiresRestart, updateSaved] = await Promise.all([
			server.saveConfiguration(draft),
			section === 2 && programmerSettingsLoaded
				? saveUpdateSettings(programmingUpdate, updateSettings)
				: Promise.resolve(true),
		]);
		if (section === 2) saveRecordSettings(recordSettings);
		setRestartRequired(requiresRestart);
		setProgrammerSettingsError(
			updateSaved ? null : "Update defaults were not saved.",
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
		fixtureLibraryOpen,
		programmerSettingsError,
		programmerSettingsLoaded,
		recordSettings,
		restartRequired,
		save,
		screenCanUndo,
		screenUndo,
		section,
		server,
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

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
