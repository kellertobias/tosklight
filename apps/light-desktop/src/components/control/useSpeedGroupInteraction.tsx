import { type ReactNode, useRef, useState } from "react";
import type { SpeedGroupId } from "../../api/types";
import { useApp } from "../../state/AppContext";
import { SoundToLightLoading } from "./SoundToLightLoading";
import { SoundToLightModal } from "./SoundToLightModal";
import {
	inactiveCaptureStatus,
	monotonicEpochMillis,
} from "./soundToLightAnalyzer";
import { useSoundToLight } from "./useSoundToLight";

const SETTINGS_HOLD_MILLIS = 650;

export interface SpeedGroupInteraction {
	beginHold: (group: SpeedGroupId, modified: boolean) => void;
	endHold: () => void;
	activate: (group: SpeedGroupId, modified: boolean) => void;
	openSettings: (group: SpeedGroupId) => void;
	settings: ReactNode;
	sound: ReturnType<typeof useSoundToLight>;
}

/**
 * Shared touch/click semantics for the software and hardware-connected Speed Group controls.
 *
 * An ordinary activation learns tempo. Shift, right-click, and a deliberate hold open settings
 * without also submitting a Learn action.
 */
export function useSpeedGroupInteraction(): SpeedGroupInteraction {
	const { state, dispatch } = useApp();
	const [settingsGroup, setSettingsGroup] = useState<SpeedGroupId | null>(null);
	const sound = useSoundToLight(true);
	const holdTimer = useRef<number | null>(null);
	const heldGroup = useRef<SpeedGroupId | null>(null);
	const suppressClick = useRef(false);

	const cancelHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
		heldGroup.current = null;
	};
	const clearArmedShift = () => {
		if (state.shiftArmed) dispatch({ type: "SET_SHIFT_ARMED", value: false });
	};
	const openSettings = (group: SpeedGroupId) => {
		cancelHold();
		clearArmedShift();
		setSettingsGroup(group);
	};
	const beginHold = (group: SpeedGroupId, modified: boolean) => {
		cancelHold();
		if (modified) return;
		heldGroup.current = group;
		holdTimer.current = window.setTimeout(() => {
			if (heldGroup.current !== group) return;
			suppressClick.current = true;
			openSettings(group);
		}, SETTINGS_HOLD_MILLIS);
	};
	const activate = (group: SpeedGroupId, modified: boolean) => {
		cancelHold();
		if (suppressClick.current) {
			suppressClick.current = false;
			return;
		}
		if (modified || state.shiftArmed) {
			openSettings(group);
			return;
		}
		void sound.action(group, {
			action: "learn",
			captured_at_millis: monotonicEpochMillis(),
		});
	};
	const selectedState = settingsGroup ? sound.states[settingsGroup] : undefined;
	const settings = settingsGroup ? (
		selectedState ? (
			<SoundToLightModal
				group={settingsGroup}
				state={selectedState}
				capture={sound.captures[settingsGroup] ?? inactiveCaptureStatus}
				controllerError={sound.error}
				onPreview={sound.setPreview}
				onSave={(configuration, source) =>
					sound.save(settingsGroup, configuration, source)
				}
				onAction={(input) => sound.action(settingsGroup, input)}
				onClose={() => setSettingsGroup(null)}
			/>
		) : (
			<SoundToLightLoading
				group={settingsGroup}
				controller={sound}
				onClose={() => setSettingsGroup(null)}
			/>
		)
	) : null;

	return {
		beginHold,
		endHold: cancelHold,
		activate,
		openSettings,
		settings,
		sound,
	};
}
