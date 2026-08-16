import { Button } from "@tosklight/ui";
import type { PlaybackDefinition } from "../../../api/types";
import {
	assignPlayback,
	isPlaybackSetClickArmed,
	offPlayback,
} from "./actions";
import type { PlaybackBankController } from "./controller";

export function PlaybackCommandTargetBadge({
	command,
}: {
	command: "record" | "set" | "off";
}) {
	return (
		<div
			className={`playback-command-target-badge ${command}`}
			aria-hidden="true"
		>
			{command.toUpperCase()} TARGET
		</div>
	);
}

export function PlaybackOffTarget({
	controller,
	playback,
}: {
	controller: PlaybackBankController;
	playback: PlaybackDefinition | null;
}) {
	if (!controller.offPending || !playback) return null;
	return (
		<Button
			className="playback-assignment-target playback-off-target"
			aria-label={`Turn off ${playback.name}`}
			onClick={() => void offPlayback(controller, playback)}
		>
			<PlaybackCommandTargetBadge command="off" />
			<b>Turn Playback Off</b>
			<small>{playback.name}</small>
		</Button>
	);
}

export function PlaybackAssignmentTarget({
	controller,
	slot,
}: {
	controller: PlaybackBankController;
	slot: number;
}) {
	if (!controller.assignmentPending) return null;
	return (
		<Button
			className="playback-assignment-target"
			aria-label={`Assign Cuelist ${controller.state.cueListSetTarget} to page ${controller.activePageNumber} playback ${slot}`}
			onClick={() => void assignPlayback(controller, slot)}
		>
			<PlaybackCommandTargetBadge command="set" />
			<b>Assign Cuelist {controller.state.cueListSetTarget}</b>
			<small>
				to playback {controller.activePageNumber}.{slot}
			</small>
		</Button>
	);
}

export function PlaybackConfigurationTarget({
	controller,
	playback,
	slot,
}: {
	controller: PlaybackBankController;
	playback: PlaybackDefinition | null;
	slot: number;
}) {
	if (controller.assignmentPending || !isPlaybackSetClickArmed(controller))
		return null;
	return (
		<div
			className="playback-assignment-target playback-configuration-target"
			aria-hidden="true"
		>
			<PlaybackCommandTargetBadge command="set" />
			<b>Configure Playback</b>
			<small>
				{controller.activePageNumber}.{slot} · {playback?.name ?? "Empty"}
			</small>
		</div>
	);
}
