import { PlaybackToolsView } from "@tosklight/ui/command";
import { useEffect } from "react";
import type { SpeedGroupId } from "../../api/types";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import { useSequenceMasterFadeMillis } from "../../features/configuration/ConfigurationState";
import { routeControlSurfaceIntentWithFeedback } from "../../features/controlSurfaceInteraction/registry";
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import { useCommandLineSurface } from "./commandLine/useCommandLineSurface";
import { PlaybackPageControl } from "./PlaybackPageControl";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";
import {
	editTargetedCommandWithSoftwareKey,
	type SoftwareKey,
} from "./softwareKeypad";
import { monotonicEpochMillis } from "./soundToLightAnalyzer";
import { formatSpeedGroupBpm } from "./speedGroupFormatting";
import { TouchTimeSurface } from "./TouchTimeSurface";
import type { SoundToLightController } from "./useSoundToLight";
import { useSpeedGroupInteraction } from "./useSpeedGroupInteraction";

export function PlaybackTools() {
	const { state, dispatch } = useApp();
	const configurationActions = useConfigurationActions();
	const sequenceMasterFadeMillis = useSequenceMasterFadeMillis();
	const command = useCommandLineSurface({ observeCommand: false });
	const speedGroups = useSpeedGroupRuntimeView();
	const speedGroupInteraction = useSpeedGroupInteraction();
	const sound = speedGroupInteraction.sound;
	useSpeedGroupKeyboardTap(sound.action);
	const pressCommandKey = (key: SoftwareKey) => {
		const currentCommand = command.read();
		if (key === "SHIFT") {
			dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed });
			return;
		}
		if (key === "SET") {
			routeControlSurfaceIntentWithFeedback({
				type: "set",
				source: "touch",
			});
			return;
		}
		if (state.shiftArmed) {
			dispatch({ type: "SET_SHIFT_ARMED", value: false });
			if (key === "DEL") {
				dispatch({
					type: "SET_MODAL",
					modal: "systemControlsOpen",
					value: true,
				});
				return;
			}
		}
		const edited = editTargetedCommandWithSoftwareKey(
			currentCommand.text,
			key,
			currentCommand.target,
			currentCommand.pristine,
		);
		void command.replace(edited.command, edited.pristine);
		if (edited.execute) void command.execute(edited.command);
	};
	const playbackSpeedGroups = (["A", "B", "C", "D", "E"] as const).map(
		(group, index) => {
			const bpm = speedGroups.ready
				? speedGroups.projection?.groups[index]?.manualBpm
				: undefined;
			return {
				id: group,
				bpm,
				display: bpm === undefined ? "—" : formatSpeedGroupBpm(bpm),
				soundEnabled: sound.states[group]?.configuration.enabled,
			};
		},
	);
	return (
		<PlaybackToolsView
			setArmed={state.playbackSetArmed}
			shiftArmed={state.shiftArmed}
			onCommandKey={pressCommandKey}
			pageControls={<PlaybackPageControl />}
			programmerFade={<ProgrammerFadeFader />}
			cueFade={
				<TouchTimeSurface
					label="Cue Fade"
					value={(sequenceMasterFadeMillis ?? 3_000) / 1_000}
					maximum={60}
					display={`${((sequenceMasterFadeMillis ?? 3_000) / 1_000).toFixed(1)} s`}
					onChange={(value) =>
						void configurationActions?.setControlTiming({
							sequence_master_fade_millis: Math.round(value * 1_000),
						})
					}
				/>
			}
			speedGroups={playbackSpeedGroups}
			onSpeedPointerDown={(group, event) =>
				speedGroupInteraction.beginHold(
					group,
					event.shiftKey || state.shiftArmed,
				)
			}
			onSpeedPointerEnd={speedGroupInteraction.endHold}
			onSpeedActivate={(group, event) =>
				speedGroupInteraction.activate(group, event.shiftKey)
			}
			onSpeedSettings={speedGroupInteraction.openSettings}
			overlays={speedGroupInteraction.settings}
		/>
	);
}

function useSpeedGroupKeyboardTap(action: SoundToLightController["action"]) {
	useEffect(() => {
		const keyboardTap = (event: Event) =>
			void action((event as CustomEvent<SpeedGroupId>).detail, {
				action: "learn",
				captured_at_millis: monotonicEpochMillis(),
			});
		window.addEventListener("light:speed-group-tap", keyboardTap);
		return () =>
			window.removeEventListener("light:speed-group-tap", keyboardTap);
	}, [action]);
}
