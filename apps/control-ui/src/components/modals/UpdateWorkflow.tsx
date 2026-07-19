import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import type {
	UpdateMenuEntry,
	UpdateMode,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
} from "../../api/types";
import { useApp } from "../../state/AppContext";
import {
	defaultUpdateSettings,
	updateTargetKey,
} from "../control/updateWorkflow";
import {
	UpdateOperationDialog,
	UpdateResultDialog,
	UpdateSettingsDialog,
	UpdateTargetMenu,
} from "./UpdateWorkflowDialogs";
import { useUpdateWorkflowActions } from "./useUpdateWorkflowActions";
import {
	type UpdateOperation,
	useUpdateWorkflowEvents,
} from "./useUpdateWorkflowEvents";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";

export {
	UpdateOperationDialog,
	UpdateSettingsDialog,
	UpdateTargetMenu,
	updatePreviewStats,
} from "./UpdateWorkflowDialogs";

export function UpdateWorkflow() {
	const server = useServer();
	const commandLine = useCommandLineSurface({ observeCommand: false });
	const { state, dispatch } = useApp();
	const [settings, setSettings] = useState<UpdateSettings>(
		defaultUpdateSettings,
	);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [operation, setOperation] = useState<UpdateOperation | null>(null);
	const [menuOpen, setMenuOpen] = useState(false);
	const [menuFilter, setMenuFilter] = useState<UpdateTargetFilter>(
		"eligible_for_update_existing",
	);
	const [menuEntries, setMenuEntries] = useState<UpdateMenuEntry[]>([]);
	const [menuModes, setMenuModes] = useState<Record<string, UpdateMode>>({});
	const [result, setResult] = useState<UpdateResult | null>(null);
	const [busy, setBusy] = useState(false);
	const [busyKey, setBusyKey] = useState<string | null>(null);
	const [localError, setLocalError] = useState<string | null>(null);
	const error = localError ?? server.error;

	const disarm = () => {
		dispatch({ type: "SET_UPDATE_ARMED", value: false });
		dispatch({ type: "SET_SHIFT_ARMED", value: false });
		if (/^UPDATE\b/i.test(commandLine.read().text.trim()))
			void commandLine.reset();
	};

	const loadMenu = async (filter: UpdateTargetFilter) => {
		setMenuFilter(filter);
		setBusy(true);
		setLocalError(null);
		const entries = await server.updateTargets(filter);
		setBusy(false);
		if (!entries)
			return setLocalError("Eligible Update targets could not be loaded.");
		setMenuEntries(entries);
		setMenuModes(
			Object.fromEntries(
				entries.map((entry) => [
					updateTargetKey(entry.target),
					entry.existing_preview.mode,
				]),
			),
		);
	};

	useUpdateWorkflowEvents({
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
	});
	const { changeOperationMode, applyOperation, saveSettings, applyMenuTarget } =
		useUpdateWorkflowActions({
			operation,
			settings,
			disarm,
			setBusy,
			setBusyKey,
			setLocalError,
			setMenuOpen,
			setOperation,
			setResult,
			setSettingsOpen,
		});

	return (
		<>
			{state.updateArmed && !operation && !busy && (
				<div className="update-armed-banner" role="status">
					UPDATE armed · touch a recordable target or enter its address
				</div>
			)}
			{busy && !operation && !settingsOpen && !menuOpen && (
				<div className="update-armed-banner busy" role="status">
					Resolving authoritative Update target…
				</div>
			)}
			{operation && (
				<UpdateOperationDialog
					operation={operation}
					busy={busy}
					error={error}
					onMode={(mode) => void changeOperationMode(mode)}
					onApply={() => void applyOperation()}
					onCancel={() => {
						setOperation(null);
						setLocalError(null);
						disarm();
					}}
				/>
			)}
			{settingsOpen && (
				<UpdateSettingsDialog
					settings={settings}
					busy={busy}
					error={error}
					onChange={setSettings}
					onSave={() => void saveSettings()}
					onCancel={() => {
						setSettingsOpen(false);
						setLocalError(null);
					}}
				/>
			)}
			{menuOpen && (
				<UpdateTargetMenu
					entries={menuEntries}
					filter={menuFilter}
					modes={menuModes}
					busyKey={busyKey}
					error={error}
					onFilter={(filter) => void loadMenu(filter)}
					onMode={(key, mode) =>
						setMenuModes((current) => ({ ...current, [key]: mode }))
					}
					onApply={(entry, mode) => void applyMenuTarget(entry, mode)}
					onCancel={() => {
						setMenuOpen(false);
						setLocalError(null);
					}}
				/>
			)}
			{result && (
				<UpdateResultDialog result={result} onClose={() => setResult(null)} />
			)}
		</>
	);
}
