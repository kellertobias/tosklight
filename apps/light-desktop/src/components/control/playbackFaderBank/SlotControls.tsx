import type { PlaybackDefinition } from "../../../api/types";
import { Button } from "../../common";
import type { VerticalTouchFaderAction } from "../VerticalTouchFader";
import { assignPlayback, isPlaybackSetClickArmed } from "./actions";
import type { PlaybackBankController } from "./controller";

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

export function SingleFaderlessButton({
	action,
	slot,
	playback,
}: {
	action: VerticalTouchFaderAction;
	slot: number;
	playback: PlaybackDefinition | null;
}) {
	const { id, label, className, ...props } = action;
	return (
		<Button
			{...props}
			aria-label={typeof label === "string" ? label : undefined}
			className={`${className ?? ""} single-button-playback-action`}
			key={id}
		>
			<b>
				{slot} · {playback?.name ?? "Empty"}
			</b>
			<span>{label}</span>
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
