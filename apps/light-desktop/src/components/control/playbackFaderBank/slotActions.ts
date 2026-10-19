import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";
import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import type { Cue, CueList, PlaybackDefinition } from "../../../api/types";
import {
	assignDynamicPlayback,
	assignGroupPlayback,
	isPlaybackControlTarget,
	isPlaybackSetClickArmed,
	openPlaybackConfiguration,
	recordPlayback,
	requestPlaybackUpdate,
	selectPlayback,
} from "./actions";
import type { PlaybackBankController } from "./controller";
import {
	buttonFeedbackClass,
	isHeldAction,
	playbackButtonLabel,
} from "./feedback";
import type { AssignedPlayback, PlaybackSnapshotActive } from "./types";

interface PlaybackActionOptions {
	controller: PlaybackBankController;
	playback: AssignedPlayback | null;
	active: PlaybackSnapshotActive | undefined;
	selected: boolean;
	slot: number;
	currentCue: Cue | null | undefined;
	buttonCount: number;
	color?: string;
}

export function buildPlaybackActions(options: PlaybackActionOptions) {
	const actions = (options.playback?.buttons ?? ["none", "none", "none"]).slice(0, options.buttonCount);
	const faderActions = actions.map((action, button) => createPlaybackAction(options, action, button));
	const primary = primaryPlaybackAction(options.playback);
	const primaryAction = createPlaybackAction(options, primary.action, primary.button, primary.direct, true);
	return { actions, faderActions, primaryAction };
}

/** A tile's normal action follows its target; a deliberately configured one-button control wins. */
function primaryPlaybackAction(playback: AssignedPlayback | null): {
	action: PlaybackDefinition["buttons"][number]; button: number; direct?: "go" | "toggle";
} {
	const configured = playback?.buttons.slice(0, playback.button_count ?? 3) ?? [];
	if (!configured.some(action => action !== "none")) return { action: "none", button: 0 };
	if (playback?.button_count === 1) return { action: configured[0], button: 0 };
	const target = playback?.target.type;
	const preferred = target === "dynamic" ? "toggle"
		: target === "cue_list" || target === "macro" || target === "timecode" ? "go" : undefined;
	const matching = preferred ? configured.indexOf(preferred) : configured.findIndex(action => action !== "none");
	if (matching >= 0) return { action: configured[matching], button: matching };
	return { action: preferred ?? "none", button: 0, direct: preferred };
}

function createPlaybackAction(
	{ controller, playback, active, selected, slot, currentCue, color }: PlaybackActionOptions,
	action: PlaybackDefinition["buttons"][number],
	button: number,
	directAction?: "go" | "toggle",
	primary = false,
): VerticalTouchFaderAction & { "data-playback-button-index": number } {
	const emptyRecordTarget =
		controller.state.storeArmed && !playback && button === 0;
	const releaseHeldAction = (event: ReactPointerEvent<HTMLButtonElement>) =>
		controller.heldActions.releaseGesture({ pointerId: event.pointerId });
	return {
		id: `${button}-${action}`,
		label:
			action === "pause" && active?.paused
				? "RESUME"
				: playbackButtonLabel(action),
		disabled:
			controller.assignmentPending ||
			!controller.runtimeActions ||
			(!playback && !emptyRecordTarget) ||
			(action === "none" && !emptyRecordTarget),
		className: buttonFeedbackClass(
			action,
			active,
			selected,
			runtimeBlackout(controller, playback),
		),
		style: playback
			? ({
					"--playback-color": color ?? playback.color ?? "#20c997",
				} as CSSProperties)
			: undefined,
		"data-playback-button-index": button + 1,
		onClick: (event) => {
			if (!playback) return;
			if (controller.state.updateArmed) {
				event.preventDefault();
				event.stopPropagation();
				requestPlaybackUpdate(playback, currentCue);
				return;
			}
			if (
				controller.setInteractionArmed ||
				controller.groupAssignmentPending
			) {
				event.preventDefault();
				event.stopPropagation();
				void assignGroupPlayback(controller, slot);
				return;
			}
			if (
				isPlaybackSetClickArmed(controller) ||
				((button === 0 || primary) && (event.shiftKey || controller.state.shiftArmed))
			) {
				event.stopPropagation();
				openPlaybackConfiguration(controller, playback, slot);
				return;
			}
			if (!isHeldAction(action) && action !== "none")
				void controller.runtimeActions?.playbackAction(
					playback.address,
					directAction ?? "button",
					{
						button: button + 1,
						pressed: true,
						control_source: {
							surface_id: controller.controlSourceId,
							control_id: button + 1,
						},
					},
				);
		},
		onPointerDown: (event) => {
			if (controller.state.updateArmed) {
				event.preventDefault();
				event.stopPropagation();
				return;
			}
			if (action !== "none" && !isHeldAction(action)) {
				const button = event.currentTarget;
				button.classList.add("playback-button-active");
				window.setTimeout(
					() => button.classList.remove("playback-button-active"),
					120,
				);
			}
			if (!playback || !isHeldAction(action)) return;
			event.currentTarget.setPointerCapture?.(event.pointerId);
			controller.heldActions.press(
				controller.controlSourceId,
				playback.address,
				button + 1,
				action,
				{ pointerId: event.pointerId },
			);
		},
		onPointerUp: releaseHeldAction,
		onPointerCancel: releaseHeldAction,
		onKeyDown: (event) => {
			if (
				!playback ||
				((button === 0 || primary) && (event.shiftKey || controller.state.shiftArmed)) ||
				controller.state.storeArmed ||
				controller.state.updateArmed ||
				controller.setInteractionArmed ||
				controller.groupAssignmentPending ||
				isPlaybackSetClickArmed(controller) ||
				!isHeldAction(action) ||
				![" ", "Enter"].includes(event.key)
			)
				return;
			event.preventDefault();
			if (!event.repeat)
				controller.heldActions.press(
					controller.controlSourceId,
					playback.address,
					button + 1,
					action,
					{ key: event.key },
				);
		},
		onKeyUp: (event) =>
			controller.heldActions.releaseGesture({ key: event.key }),
	};
}

function runtimeBlackout(
	controller: PlaybackBankController,
	playback: AssignedPlayback | null,
) {
	const projection = playback
		? controller.runtimeProjections.get(playback.address.position)
		: undefined;
	return Boolean(
		projection?.target === "grand_master" && projection.runtime.blackout,
	);
}

export function createSlotInterceptors(
	controller: PlaybackBankController,
	playback: AssignedPlayback | null,
	cueList: CueList | null | undefined,
	slot: number,
	currentCue: Cue | null | undefined,
	buttonCount: number,
) {
	const isRecordTarget = (target: EventTarget | null) =>
		!controller.hardware || hardwareRecordTarget(target, buttonCount);
	const interceptPointer = (event: ReactPointerEvent<HTMLElement>) => {
		if (controller.state.updateArmed) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (controller.state.storeArmed) {
			if (!isRecordTarget(event.target)) return;
			// Recording is committed by the click interceptor. Cancelling pointer-down
			// can suppress that compatibility click in the desktop webview, forcing a
			// second press before the Record target sees it.
			event.stopPropagation();
			return;
		}
		if (controller.setInteractionArmed || controller.groupAssignmentPending) {
			event.stopPropagation();
			return;
		}
		const firstButton = (event.target as Element).closest(
			'[data-playback-button-index="1"], .playback-tile-action',
		);
		if (
			!isPlaybackSetClickArmed(controller) &&
			!(firstButton && (event.shiftKey || controller.state.shiftArmed))
		)
			return;
		event.preventDefault();
		event.stopPropagation();
		openPlaybackConfiguration(controller, playback, slot);
	};
	const interceptClick = (event: ReactMouseEvent<HTMLElement>) => {
		if (controller.groupAssignmentPending || controller.setInteractionArmed) {
			event.preventDefault();
			event.stopPropagation();
			void assignGroupPlayback(controller, slot);
			return;
		}
		if (controller.state.storeArmed) {
			if (!isRecordTarget(event.target)) return;
			event.preventDefault();
			event.stopPropagation();
			void recordPlayback(controller, event, playback, cueList, slot);
			return;
		}
		if (controller.dynamicAssignmentPending && playback) {
			event.preventDefault();
			event.stopPropagation();
			void assignDynamicPlayback(controller, slot);
			return;
		}
		if (!controller.hardware) {
			if (controller.state.updateArmed) {
				event.preventDefault();
				event.stopPropagation();
				requestPlaybackUpdate(playback, currentCue);
				return;
			}
			const firstButton = (event.target as Element).closest(
				'[data-playback-button-index="1"], .playback-tile-action',
			);
			if (
				isPlaybackSetClickArmed(controller) ||
				(firstButton && (event.shiftKey || controller.state.shiftArmed))
			) {
				event.preventDefault();
				event.stopPropagation();
				openPlaybackConfiguration(controller, playback, slot);
				return;
			}
			if (controller.selectionPending && playback) {
				event.preventDefault();
				event.stopPropagation();
				void selectPlayback(controller, playback);
			}
			return;
		}
		if (isPlaybackControlTarget(event.target)) return;
		if (controller.state.updateArmed) {
			event.preventDefault();
			event.stopPropagation();
			requestPlaybackUpdate(playback, currentCue);
			return;
		}
		if (isPlaybackSetClickArmed(controller)) {
			event.preventDefault();
			event.stopPropagation();
			openPlaybackConfiguration(controller, playback, slot);
		}
	};
	return { interceptPointer, interceptClick };
}

function hardwareRecordTarget(target: EventTarget | null, buttonCount: number) {
	if (!(target instanceof Element)) return false;
	if (buttonCount > 0)
		return Boolean(target.closest('[data-playback-button-index="1"]'));
	return !target.closest("button, input, .hardware-playback-controls");
}
