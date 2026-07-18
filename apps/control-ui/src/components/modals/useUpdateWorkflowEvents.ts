import { type Dispatch, type SetStateAction, useEffect } from "react";
import { useServer } from "../../api/ServerContext";
import type {
	UpdatePreview,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetRequest,
} from "../../api/types";
import { useApp } from "../../state/AppContext";
import {
	configuredUpdateMode,
	defaultUpdateSettings,
	targetFamilyLabel,
	UPDATE_ARMED_EVENT,
	UPDATE_SETTINGS_EVENT,
	UPDATE_TARGET_EVENT,
	UPDATE_TARGET_MENU_EVENT,
} from "../control/updateWorkflow";

export type UpdateOperation = {
	request: UpdateTargetRequest;
	preview: UpdatePreview;
};

interface UpdateWorkflowEventOptions {
	operation: UpdateOperation | null;
	busy: boolean;
	disarm: () => void;
	loadMenu: (filter: UpdateTargetFilter) => Promise<void>;
	setBusy: Dispatch<SetStateAction<boolean>>;
	setLocalError: Dispatch<SetStateAction<string | null>>;
	setMenuOpen: Dispatch<SetStateAction<boolean>>;
	setOperation: Dispatch<SetStateAction<UpdateOperation | null>>;
	setResult: Dispatch<SetStateAction<UpdateResult | null>>;
	setSettings: Dispatch<SetStateAction<UpdateSettings>>;
	setSettingsOpen: Dispatch<SetStateAction<boolean>>;
}

export function useUpdateWorkflowEvents({
	operation,
	busy,
	disarm,
	loadMenu,
	setBusy,
	setLocalError,
	setMenuOpen,
	setOperation,
	setResult,
	setSettings,
	setSettingsOpen,
}: UpdateWorkflowEventOptions) {
	const server = useServer();
	const { state, dispatch } = useApp();

	useEffect(() => {
		const selectTarget = (event: Event) => {
			if (!state.updateArmed || operation || busy) return;
			const request = (event as CustomEvent<UpdateTargetRequest>).detail;
			void (async () => {
				setBusy(true);
				setLocalError(null);
				const nextSettings = await server.updateSettings();
				if (!nextSettings) {
					setBusy(false);
					setLocalError("Update settings could not be loaded.");
					disarm();
					return;
				}
				setSettings(nextSettings);
				const mode = configuredUpdateMode(nextSettings, request);
				if (!nextSettings.show_update_modal_on_touch) {
					const applied = await server.applyUpdate(request, mode);
					setBusy(false);
					disarm();
					if (applied) setResult(applied);
					else setLocalError("Update failed; no show data was changed.");
					return;
				}
				const preview = await server.previewUpdate(request, mode);
				setBusy(false);
				if (!preview) {
					setLocalError("Update preview failed; no show data was changed.");
					disarm();
					return;
				}
				setOperation({ request, preview });
				server.setCommandLine(
					`UPDATE ${targetFamilyLabel(preview.target).toUpperCase()} ${preview.target.name}`,
					false,
				);
			})();
		};
		const openSettings = () => {
			void (async () => {
				disarm();
				setLocalError(null);
				setBusy(true);
				const next = await server.updateSettings();
				setBusy(false);
				setSettings(next ?? defaultUpdateSettings);
				if (!next) {
					setLocalError(
						"Update settings could not be loaded; deterministic defaults are shown.",
					);
				}
				setSettingsOpen(true);
			})();
		};
		const openMenu = () => {
			disarm();
			setMenuOpen(true);
			void loadMenu("eligible_for_update_existing");
		};
		const synchronizeArmed = (event: Event) => {
			const armed = Boolean((event as CustomEvent<boolean>).detail);
			dispatch({ type: "SET_UPDATE_ARMED", value: armed });
			if (armed) server.setCommandLine("UPDATE ", false);
			else if (/^UPDATE\b/i.test(server.commandLine.trim())) {
				server.resetCommandLine();
			}
		};
		window.addEventListener(UPDATE_TARGET_EVENT, selectTarget);
		window.addEventListener(UPDATE_ARMED_EVENT, synchronizeArmed);
		window.addEventListener(UPDATE_SETTINGS_EVENT, openSettings);
		window.addEventListener(UPDATE_TARGET_MENU_EVENT, openMenu);
		return () => {
			window.removeEventListener(UPDATE_TARGET_EVENT, selectTarget);
			window.removeEventListener(UPDATE_ARMED_EVENT, synchronizeArmed);
			window.removeEventListener(UPDATE_SETTINGS_EVENT, openSettings);
			window.removeEventListener(UPDATE_TARGET_MENU_EVENT, openMenu);
		};
	}, [
		state.updateArmed,
		operation,
		busy,
		server,
		disarm,
		loadMenu,
		setBusy,
		setLocalError,
		setMenuOpen,
		setOperation,
		setResult,
		setSettings,
		setSettingsOpen,
		dispatch,
	]);
}
