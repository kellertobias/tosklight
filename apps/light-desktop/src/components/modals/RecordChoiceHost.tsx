import { useEffect, useState } from "react";
import type { RecordUpdateOption, UpdateSettings } from "../../api/types";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import {
	armedCommandLine,
	commandLineOption,
	migrateLegacyRecordDefaults,
} from "../../features/recordUpdateOptions/options";
import { useApp } from "../../state/AppContext";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { RecordUpdateChoiceModal } from "./RecordUpdateChoiceModal";

/** Owns the RECORD RECORD choice and the one-time move of the retired browser Record defaults. */
export function RecordChoiceHost() {
	const { state, dispatch } = useApp();
	const update = useProgrammingUpdate();
	const command = useCommandLineSurface({ observeCommand: false });
	const [settings, setSettings] = useState<UpdateSettings | null>(null);
	const [initialOption, setInitialOption] =
		useState<RecordUpdateOption | null>(null);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const open = state.recordChoiceOpen;
	const scopeKey = update?.scopeKey ?? null;

	// biome-ignore lint/correctness/useExhaustiveDependencies: migrate once per desk scope
	useEffect(() => {
		if (update) void migrateLegacyRecordDefaults(update).catch(() => false);
	}, [scopeKey]);

	// biome-ignore lint/correctness/useExhaustiveDependencies: reload each time the modal opens
	useEffect(() => {
		if (!open) return;
		let current = true;
		setSettings(null);
		setError(null);
		setBusy(false);
		setInitialOption(commandLineOption(command.read().text, "RECORD"));
		void (async () => {
			try {
				const next = (await update?.loadSettings()) ?? null;
				if (!current) return;
				setSettings(next);
				if (!next) setError("The Record default could not be loaded.");
			} catch (reason) {
				if (current) setError(errorMessage(reason));
			}
		})();
		return () => {
			current = false;
		};
	}, [open]);

	if (!open) return null;
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "recordChoiceOpen", value: false });
	const confirm = async (option: RecordUpdateOption, setAsDefault: boolean) => {
		if (!settings) return;
		let storedDefault = settings.record_default;
		if (setAsDefault && option !== storedDefault) {
			setBusy(true);
			setError(null);
			try {
				const saved = await update?.saveSettings({
					...settings,
					record_default: option,
				});
				if (!saved) {
					setBusy(false);
					setError("The Record default could not be saved.");
					return;
				}
				storedDefault = saved.record_default;
			} catch (reason) {
				setBusy(false);
				setError(errorMessage(reason));
				return;
			}
			setBusy(false);
		}
		if (state.cueListSetArmed)
			dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		dispatch({ type: "SET_STORE_ARMED", value: true });
		await command.replace(
			armedCommandLine(command.read().text, "RECORD", option, storedDefault),
			false,
		);
		close();
	};
	return (
		<RecordUpdateChoiceModal
			kind="record"
			storedDefault={settings?.record_default ?? null}
			initialOption={initialOption}
			busy={busy}
			error={error}
			onConfirm={(option, setAsDefault) => void confirm(option, setAsDefault)}
			onCancel={close}
		/>
	);
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
