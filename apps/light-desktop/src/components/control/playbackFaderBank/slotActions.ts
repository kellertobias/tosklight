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
import type { PlaybackSnapshotActive } from "./types";

export function buildPlaybackActions({
	controller,
	playback,
	active,
	selected,
	slot,
	currentCue,
	buttonCount,
	color,
}: {
	controller: PlaybackBankController;
	playback: PlaybackDefinition | null;
	active: PlaybackSnapshotActive | undefined;
	selected: boolean;
	slot: number;
	currentCue: Cue | null | undefined;
	buttonCount: number;
	color?: string;
}) {
	const actions = (playback?.buttons ?? ["none", "none", "none"]).slice(
		0,
		buttonCount,
	);
	const faderActions: VerticalTouchFaderAction[] = actions.map(
		(action, button) => {
			const emptyRecordTarget =
				controller.state.storeArmed && !playback && button === 0;
			const releaseHeldAction = () =>
				controller.heldActions.releaseButton(slot, button + 1);
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
						(button === 0 && (event.shiftKey || controller.state.shiftArmed))
					) {
						event.stopPropagation();
						openPlaybackConfiguration(controller, playback, slot);
						return;
					}
					if (!isHeldAction(action) && action !== "none")
						void controller.runtimeActions?.poolPlaybackAction(
							playback.number,
							"button",
							{
								button: button + 1,
								pressed: true,
								surface: "physical",
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
						slot,
						playback.number,
						button + 1,
						action,
					);
				},
				onPointerUp: releaseHeldAction,
				onPointerCancel: releaseHeldAction,
				onLostPointerCapture: releaseHeldAction,
			};
		},
	);
	return { actions, faderActions };
}

function runtimeBlackout(
	controller: PlaybackBankController,
	playback: PlaybackDefinition | null,
) {
	const projection = playback
		? controller.runtimeProjections.get(playback.number)
		: undefined;
	return Boolean(
		projection?.target === "grand_master" && projection.runtime.blackout,
	);
}

export function createSlotInterceptors(
	controller: PlaybackBankController,
	playback: PlaybackDefinition | null,
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
			'[data-playback-button-index="1"]',
		);
		if (
			!isPlaybackSetClickArmed(controller) &&
			!(firstButton && controller.state.shiftArmed)
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
				'[data-playback-button-index="1"]',
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
