import { type Dispatch, type SetStateAction, useEffect, useRef } from "react";
import type {
	UpdatePreview,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetRequest,
} from "../../api/types";
import type { UpdatePreviewAuthority } from "../../features/programmingUpdate/contracts";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import { useApp } from "../../state/AppContext";
import type { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import {
	configuredUpdateMode,
	defaultUpdateSettings,
	requestFromUpdateIdentity,
	targetFamilyLabel,
	UPDATE_ARMED_EVENT,
	UPDATE_SETTINGS_EVENT,
	UPDATE_TARGET_EVENT,
	UPDATE_TARGET_MENU_EVENT,
} from "../control/updateWorkflow";

export type UpdateOperation = {
	request: UpdateTargetRequest;
	preview: UpdatePreview;
	authority: UpdatePreviewAuthority;
};

interface UpdateWorkflowEventOptions {
	commandLine: ReturnType<typeof useCommandLineSurface>;
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
	commandLine,
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
	const update = useProgrammingUpdate();
	const { state, dispatch } = useApp();
	const scopeKeyRef = useRef(update?.scopeKey ?? "unavailable");
	scopeKeyRef.current = update?.scopeKey ?? "unavailable";

	useEffect(() => {
		const selectTarget = (event: Event) => {
			if (!state.updateArmed || operation || busy) return;
			const request = (event as CustomEvent<UpdateTargetRequest>).detail;
			void (async () => {
				const requestedScope = update?.scopeKey ?? "unavailable";
				setBusy(true);
				setLocalError(null);
				try {
					const nextSettings = await update?.loadSettings();
					if (scopeKeyRef.current !== requestedScope) return;
					if (!nextSettings) {
						setBusy(false);
						setLocalError("Update settings could not be loaded.");
						disarm();
						return;
					}
					setSettings(nextSettings);
					const mode = configuredUpdateMode(nextSettings, request);
					if (!nextSettings.show_update_modal_on_touch) {
						const applied = await update?.applyDirect(request, mode);
						if (scopeKeyRef.current !== requestedScope) return;
						setBusy(false);
						disarm();
						if (applied) setResult(applied.result);
						else setLocalError("Update failed; no show data was changed.");
						return;
					}
					const authority = await update?.preview(request, mode);
					if (scopeKeyRef.current !== requestedScope) return;
					setBusy(false);
					if (!authority) {
						setLocalError("Update preview failed; no show data was changed.");
						disarm();
						return;
					}
					setOperation({
						request: requestFromUpdateIdentity(authority.preview.target),
						preview: authority.preview,
						authority,
					});
					void commandLine.replace(
						`UPDATE ${targetFamilyLabel(authority.preview.target).toUpperCase()} ${authority.preview.target.name}`,
						false,
					);
				} catch (reason) {
					if (scopeKeyRef.current !== requestedScope) return;
					setBusy(false);
					setLocalError(errorMessage(reason));
					disarm();
				}
			})();
		};
		const openSettings = () => {
			void (async () => {
				const requestedScope = update?.scopeKey ?? "unavailable";
				disarm();
				setLocalError(null);
				setBusy(true);
				try {
					const next = await update?.loadSettings();
					if (scopeKeyRef.current !== requestedScope) return;
					setBusy(false);
					setSettings(next ?? defaultUpdateSettings);
					if (!next)
						setLocalError(
							"Update settings could not be loaded; deterministic defaults are shown.",
						);
					setSettingsOpen(true);
				} catch (reason) {
					if (scopeKeyRef.current !== requestedScope) return;
					setBusy(false);
					setSettings(defaultUpdateSettings);
					setLocalError(errorMessage(reason));
					setSettingsOpen(true);
				}
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
			if (armed) void commandLine.replace("UPDATE ", false);
			else if (/^UPDATE\b/i.test(commandLine.read().text.trim())) {
				void commandLine.reset();
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
		update,
		commandLine,
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

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
