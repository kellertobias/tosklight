import { type CSSProperties, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useSequenceMasterFadeMillis } from "../../features/configuration/ConfigurationState";
import { useConfigurationActions } from "../../features/configuration/ConfigurationActionsProvider";
import type { SpeedGroupId } from "../../api/types";
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { useCommandLineSurface } from "./commandLine/useCommandLineSurface";
import { PlaybackPageControl } from "./PlaybackPageControl";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";
import { SoundToLightModal } from "./SoundToLightModal";
import {
	editTargetedCommandWithSoftwareKey,
	type SoftwareKey,
} from "./softwareKeypad";
import {
	inactiveCaptureStatus,
	monotonicEpochMillis,
} from "./soundToLightAnalyzer";
import { TouchTimeSurface } from "./TouchTimeSurface";
import {
	type SoundToLightController,
	useSoundToLight,
} from "./useSoundToLight";

export function PlaybackTools() {
	const { state, dispatch } = useApp();
	const configurationActions = useConfigurationActions();
	const sequenceMasterFadeMillis = useSequenceMasterFadeMillis();
	const command = useCommandLineSurface({ observeCommand: false });
	const speedGroups = useSpeedGroupRuntimeView();
	const [soundGroup, setSoundGroup] = useState<SpeedGroupId | null>(null);
	const sound = useSoundToLight(true);
	const holdTimer = useRef<number | null>(null);
	const heldGroup = useRef<SpeedGroupId | null>(null);
	const suppressClick = useRef(false);
	useSpeedGroupKeyboardTap(sound.action);
	const pressCommandKey = (key: SoftwareKey) => {
		const currentCommand = command.read();
		if (key === "SHIFT") {
			dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed });
			return;
		}
		if (key === "SET") {
			dispatch({
				type: "SET_PLAYBACK_SET_ARMED",
				value: !state.playbackSetArmed,
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
	const selectedSoundState = soundGroup ? sound.states[soundGroup] : undefined;
	const cancelHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
		heldGroup.current = null;
	};
	const beginHold = (group: SpeedGroupId, modified: boolean) => {
		cancelHold();
		if (modified) return;
		heldGroup.current = group;
		holdTimer.current = window.setTimeout(() => {
			if (heldGroup.current !== group) return;
			suppressClick.current = true;
			setSoundGroup(group);
			cancelHold();
		}, 650);
	};
	const activateSpeedGroup = (
		group: SpeedGroupId,
		modified: boolean,
	) => {
		cancelHold();
		if (suppressClick.current) {
			suppressClick.current = false;
			return;
		}
		if (modified || state.shiftArmed) {
			if (state.shiftArmed)
				dispatch({ type: "SET_SHIFT_ARMED", value: false });
			setSoundGroup(group);
			return;
		}
		void sound.action(group, {
			action: "learn",
			captured_at_millis: monotonicEpochMillis(),
		});
	};
	return (
		<div className="playback-tools">
			<PlaybackCommandKeys
				setArmed={state.playbackSetArmed}
				shiftArmed={state.shiftArmed}
				onPress={pressCommandKey}
			/>
			<PlaybackPageControl />
			<ProgrammerFadeFader />
			<div className="cue-fade-master">
				<TouchTimeSurface
					label="Cue Fade"
					value={
						(sequenceMasterFadeMillis ?? 3_000) / 1_000
					}
					maximum={60}
					display={`${((sequenceMasterFadeMillis ?? 3_000) / 1_000).toFixed(1)} s`}
					onChange={(value) =>
						void configurationActions?.setControlTiming({
							sequence_master_fade_millis: Math.round(value * 1_000),
						})
					}
				/>
			</div>
			<SpeedGroupControls
				bpms={
					speedGroups.ready
						? speedGroups.projection?.groups.map((group) => group.manualBpm) ?? []
						: []
				}
				controller={sound}
				shiftArmed={state.shiftArmed}
				onHoldStart={beginHold}
				onHoldEnd={cancelHold}
				onActivate={activateSpeedGroup}
			/>
			{soundGroup && selectedSoundState && (
				<SoundToLightModal
					group={soundGroup}
					state={selectedSoundState}
					capture={sound.captures[soundGroup] ?? inactiveCaptureStatus}
					controllerError={sound.error}
					onPreview={sound.setPreview}
					onSave={(configuration, source) =>
						sound.save(soundGroup, configuration, source)
					}
					onAction={(input) => sound.action(soundGroup, input)}
					onClose={() => setSoundGroup(null)}
				/>
			)}
			{soundGroup && !selectedSoundState && (
				<SoundToLightLoading
					group={soundGroup}
					controller={sound}
					onClose={() => setSoundGroup(null)}
				/>
			)}
		</div>
	);
}

function PlaybackCommandKeys({
	setArmed,
	shiftArmed,
	onPress,
}: {
	setArmed: boolean;
	shiftArmed: boolean;
	onPress: (key: SoftwareKey) => void;
}) {
	return <div className="playback-command-keys">
		{(["SET", "CPY", "MOV", "DEL", "SHIFT"] as const).map((key) => (
			<Button
				className={
					(key === "SET" && setArmed) || (key === "SHIFT" && shiftArmed)
						? "active"
						: ""
				}
				data-keypad-key={key}
				key={key}
				onClick={() => onPress(key)}
			>
				{key}
			</Button>
		))}
	</div>;
}

function SpeedGroupControls({
	bpms,
	controller,
	shiftArmed,
	onHoldStart,
	onHoldEnd,
	onActivate,
}: {
	bpms: Array<number | undefined>;
	controller: SoundToLightController;
	shiftArmed: boolean;
	onHoldStart: (group: SpeedGroupId, modified: boolean) => void;
	onHoldEnd: () => void;
	onActivate: (group: SpeedGroupId, modified: boolean) => void;
}) {
	return (
		<div className="speed-group-stack">
			{(["A", "B", "C", "D", "E"] as const).map((group, index) => {
				const bpm = bpms[index];
				const displayBpm =
					bpm === undefined
						? "—"
						: Number.isInteger(bpm)
							? String(bpm)
							: bpm.toFixed(1);
				return (
					<Button
						style={
							bpm === undefined
								? undefined
								: ({ "--bpm": bpm } as CSSProperties)
						}
						className={`active ${controller.states[group]?.configuration.enabled ? "sound-enabled" : ""}`}
						aria-label={
							bpm === undefined
								? `Speed group ${group}, loading`
								: `Speed group ${group}, ${displayBpm} BPM`
						}
						title={`Tap Speed Group ${group}; Shift or hold for settings`}
						key={group}
						onPointerDown={(event) =>
							onHoldStart(group, event.shiftKey || shiftArmed)
						}
						onPointerUp={onHoldEnd}
						onPointerCancel={onHoldEnd}
						onPointerLeave={onHoldEnd}
						onClick={(event) => onActivate(group, event.shiftKey)}
					>
						<strong className="speed-group-label">{group}</strong>
						<span className="speed-group-value">{displayBpm}</span>
						<small className="speed-group-unit">BPM</small>
					</Button>
				);
			})}
		</div>
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

function SoundToLightLoading({
	group,
	controller,
	onClose,
}: {
	group: SpeedGroupId;
	controller: SoundToLightController;
	onClose: () => void;
}) {
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`Speed Group ${group} Sound to Light`}
			>
				<Button
					className="modal-close"
					aria-label="Close Sound-to-Light configuration"
					onClick={onClose}
				>
					×
				</Button>
				<h3>Speed Group {group} · Sound to Light</h3>
				<p>
					{controller.loading
						? "Loading Speed Group configuration…"
						: (controller.error ??
							"Speed Group configuration is not available.")}
				</p>
			</section>
		</div>,
		document.body,
	);
}
