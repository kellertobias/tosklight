import { useState } from "react";
import type { RecordUpdateOption, UpdateSettings } from "../../api/types";
import type { ProgrammingUpdateCapability } from "../../features/programmingUpdate/contracts";
import {
	armedCommandLine,
	commandLineOption,
} from "../../features/recordUpdateOptions/options";
import { useApp } from "../../state/AppContext";
import type { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";

interface UpdateChoiceState {
	open: boolean;
	settings: UpdateSettings | null;
	initialOption: RecordUpdateOption | null;
	busy: boolean;
	error: string | null;
}

const CLOSED: UpdateChoiceState = {
	open: false,
	settings: null,
	initialOption: null,
	busy: false,
	error: null,
};

/** UPDATE UPDATE: choose how this Update stores the programmer, optionally as the default. */
export function useUpdateChoice({
	update,
	commandLine,
	openTargets,
}: {
	update: ProgrammingUpdateCapability | null;
	commandLine: ReturnType<typeof useCommandLineSurface>;
	openTargets: () => void;
}) {
	const { state, dispatch } = useApp();
	const [choice, setChoice] = useState<UpdateChoiceState>(CLOSED);

	const open = async () => {
		setChoice({
			...CLOSED,
			open: true,
			initialOption: commandLineOption(commandLine.read().text, "UPDATE"),
		});
		try {
			const settings = (await update?.loadSettings()) ?? null;
			setChoice((current) =>
				current.open
					? {
							...current,
							settings,
							error: settings
								? null
								: "The Update default could not be loaded.",
						}
					: current,
			);
		} catch (reason) {
			setChoice((current) =>
				current.open ? { ...current, error: errorMessage(reason) } : current,
			);
		}
	};

	const close = () => setChoice(CLOSED);

	const confirm = async (option: RecordUpdateOption, setAsDefault: boolean) => {
		const settings = choice.settings;
		if (!settings) return;
		let storedDefault = settings.update_default;
		if (setAsDefault && option !== storedDefault) {
			setChoice((current) => ({ ...current, busy: true, error: null }));
			const saved = await update
				?.saveSettings({ ...settings, update_default: option })
				.catch((reason: unknown) => {
					setChoice((current) => ({
						...current,
						busy: false,
						error: errorMessage(reason),
					}));
					return undefined;
				});
			if (saved === undefined) return;
			if (!saved) {
				setChoice((current) => ({
					...current,
					busy: false,
					error: "The Update default could not be saved.",
				}));
				return;
			}
			storedDefault = saved.update_default;
		}
		if (state.cueListSetArmed)
			dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		if (state.playbackSetArmed)
			dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
		if (state.presetSetArmed)
			dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
		dispatch({ type: "SET_UPDATE_ARMED", value: true });
		await commandLine.replace(
			armedCommandLine(
				commandLine.read().text,
				"UPDATE",
				option,
				storedDefault,
			),
			false,
		);
		close();
	};

	return {
		isOpen: choice.open,
		initialOption: choice.initialOption,
		busy: choice.busy,
		error: choice.error,
		storedDefault: choice.settings?.update_default ?? null,
		show: open,
		close,
		confirm,
		targets: () => {
			close();
			openTargets();
		},
	};
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
