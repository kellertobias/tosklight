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

export function useSetupWindowController() {
	const server = useServer();
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
		setRecordSettings(loadRecordSettings());
		setProgrammerSettingsError(null);
		void server.updateSettings().then((settings) => {
			if (!active) return;
			setUpdateSettings(settings ?? defaultUpdateSettings);
			setProgrammerSettingsLoaded(true);
			if (!settings) {
				setProgrammerSettingsError(
					"Update defaults could not be loaded; deterministic defaults are shown.",
				);
			}
		});
		return () => {
			active = false;
		};
	}, [section]);

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
				? server.saveUpdateSettings(updateSettings)
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
