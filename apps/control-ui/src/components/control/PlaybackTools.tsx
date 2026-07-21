import { type CSSProperties, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
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
	const server = useServer();
	const command = useCommandLineSurface({ observeCommand: false });
	const speedGroups = useSpeedGroupRuntimeView();
	const [soundGroup, setSoundGroup] = useState<SpeedGroupId | null>(null);
	const sound = useSoundToLight(soundGroup !== null);
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
	return (
		<div className="playback-tools">
			<div className="playback-command-keys">
				{(["SET", "CPY", "MOV", "DEL", "SHIFT"] as const).map((key) => (
					<Button
						className={
							(key === "SET" && state.playbackSetArmed) ||
							(key === "SHIFT" && state.shiftArmed)
								? "active"
								: ""
						}
						data-keypad-key={key}
						key={key}
						onClick={() => pressCommandKey(key)}
					>
						{key}
					</Button>
				))}
			</div>
			<PlaybackPageControl />
			<ProgrammerFadeFader />
			<div className="cue-fade-master">
				<TouchTimeSurface
					label="Cue Fade"
					value={
						(server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000
					}
					maximum={60}
					display={`${((server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`}
					onChange={(value) =>
						void server.setControlTiming({
							sequence_master_fade_millis: Math.round(value * 1_000),
						})
					}
				/>
			</div>
			<div className="speed-group-stack">
				{(["A", "B", "C", "D", "E"] as const).map((group, index) => {
					const speedState = sound.states[group];
					const bpm = speedGroups.ready
						? speedGroups.projection?.groups[index]?.manualBpm
						: undefined;
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
							className={`active ${speedState?.configuration.enabled ? "sound-enabled" : ""}`}
							aria-label={
								bpm === undefined
									? `Speed group ${group}, loading`
									: `Speed group ${group}, ${displayBpm} BPM`
							}
							title={`Open Speed Group ${group} Sound-to-Light configuration`}
							key={group}
							onClick={() => setSoundGroup(group)}
						>
							<strong className="speed-group-label">{group}</strong>
							<span className="speed-group-value">{displayBpm}</span>
							<small className="speed-group-unit">BPM</small>
						</Button>
					);
				})}
			</div>
			{soundGroup && selectedSoundState && (
				<SoundToLightModal
					group={soundGroup}
					state={selectedSoundState}
					capture={sound.captures[soundGroup] ?? inactiveCaptureStatus}
					permission={sound.permission}
					devices={sound.devices}
					deviceId={sound.deviceIds[soundGroup] ?? ""}
					controllerError={sound.error}
					onDeviceChange={(deviceId) => sound.setDevice(soundGroup, deviceId)}
					onRefreshInputs={sound.refreshInputs}
					onPreview={sound.setPreview}
					onSave={(configuration) => sound.save(soundGroup, configuration)}
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
