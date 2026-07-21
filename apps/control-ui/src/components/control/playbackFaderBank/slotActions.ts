import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import type { Cue, PlaybackDefinition } from "../../../api/types";
import type { VerticalTouchFaderAction } from "../VerticalTouchFader";
import {
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
}: {
	controller: PlaybackBankController;
	playback: PlaybackDefinition | null;
	active: PlaybackSnapshotActive | undefined;
	selected: boolean;
	slot: number;
	currentCue: Cue | null | undefined;
	buttonCount: number;
}) {
	const actions = (playback?.buttons ?? ["none", "none", "none"]).slice(
		0,
		buttonCount,
	);
	const faderActions: VerticalTouchFaderAction[] = actions.map(
		(action, button) => {
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
					!playback ||
					action === "none",
				className: buttonFeedbackClass(
					action,
					active,
					selected,
					runtimeBlackout(controller, playback),
				),
				style: playback
					? ({
							"--playback-color": playback.color ?? "#20c997",
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
	slot: number,
	currentCue: Cue | null | undefined,
) {
	const interceptPointer = (event: ReactPointerEvent<HTMLElement>) => {
		if (controller.state.updateArmed) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (controller.state.storeArmed) {
			event.preventDefault();
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
		if (controller.state.storeArmed) {
			void recordPlayback(controller, event, playback, slot);
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
