import type { PlaybackDefinition } from "../../../api/types";
import { Button } from "@tosklight/ui";
import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";
import { assignPlayback, isPlaybackSetClickArmed } from "./actions";
import type { PlaybackBankController } from "./controller";
import type { PlaybackSnapshotActive } from "./types";

export function PlaybackActionButtons({
	actions,
}: {
	actions: VerticalTouchFaderAction[];
}) {
	return actions.map(({ id, label, ...props }) => (
		<Button {...props} key={id}>
			{label}
		</Button>
	));
}

export function PlaybackRuntimeStatus({
	active,
}: {
	active: PlaybackSnapshotActive | undefined;
}) {
	const status = active?.flash
		? ["flash", "FLASH HELD"]
		: active?.swap_active
			? ["swap", "SWAP HELD"]
			: active?.loaded_cue_number != null
				? ["loaded", "LOADED"]
				: null;
	if (!status) return null;
	return (
		<span
			className={`playback-status playback-status-${status[0]}`}
			role="status"
		>
			{status[1]}
		</span>
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
			<b>Configure Playback</b>
			<small>
				{controller.activePageNumber}.{slot} · {playback?.name ?? "Empty"}
			</small>
		</div>
	);
}

export function PlaybackRepresentation({
	controller,
	playback,
	slot,
}: {
	controller: PlaybackBankController;
	playback: PlaybackDefinition | null;
	slot: number;
}) {
	return (
		<Button
			className="playback-software-representation"
			aria-label={`Playback representation page ${controller.activePageNumber} playback ${slot}`}
		>
			<b>
				{slot} · {playback?.name ?? "Empty"}
			</b>
		</Button>
	);
}
