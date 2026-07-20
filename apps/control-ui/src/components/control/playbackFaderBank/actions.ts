import type { MouseEvent as ReactMouseEvent } from "react";
import type { Cue, PlaybackDefinition } from "../../../api/types";
import { isSetContextClick } from "../../../disableContextMenu";
import { normalizePlaybackTopology } from "../PlaybackConfigurationModal";
import { cueUpdateTarget, requestUpdateTarget } from "../updateWorkflow";
import type { PlaybackBankController } from "./controller";
import { emptyConfiguration } from "./feedback";
import { loadRecordSettings } from "../../setup/ProgrammerDefaults";

export function isPlaybackSetClickArmed(controller: PlaybackBankController) {
	const { state } = controller;
	return (
		state.playbackSetArmed ||
		(state.cueListSetArmed && state.cueListSetTarget == null)
	);
}

export function openPlaybackConfiguration(
	controller: PlaybackBankController,
	playback: PlaybackDefinition | null,
	slot: number,
) {
	const { server, dispatch, activePageNumber, buttons, cueLists } = controller;
	const fallbackButtons = Math.max(
		0,
		Math.min(3, buttons ?? server.playbacks?.desk.buttons ?? 3),
	);
	controller.setConfiguration({
		playback: normalizePlaybackTopology(
			playback ??
				emptyConfiguration(
					activePageNumber,
					slot,
					fallbackButtons,
					true,
					cueLists[0]?.id ?? "",
				),
			fallbackButtons,
			true,
		),
		page: activePageNumber,
		slot,
		empty: !playback,
	});
	dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
	dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	dispatch({ type: "SET_SHIFT_ARMED", value: false });
}

export async function selectPlayback(
	controller: PlaybackBankController,
	playback: PlaybackDefinition,
) {
	await controller.server.poolPlaybackAction(playback.number, "select");
	if (controller.selectionPending) void controller.command.reset();
}

export async function recordPlayback(
	controller: PlaybackBankController,
	event: ReactMouseEvent,
	playback: PlaybackDefinition | null,
	slot: number,
) {
	if (!controller.state.storeArmed) return;
	event.preventDefault();
	event.stopPropagation();
	const settings = loadRecordSettings();
	const outcome = await controller.cueRecording?.record({
		target: {
			kind: "page_slot",
			page: controller.activePageNumber,
			slot,
		},
		operation:
			settings.mergeActiveCue ? "merge" : "overwrite",
		timing: {},
		cueOnly: settings.cueOnly,
		capturePolicy: "current_capture",
		activationPolicy: "go_to_if_normal",
	});
	if (!outcome) return;
	controller.dispatch({ type: "SET_STORE_ARMED", value: false });
	await controller.command.reset();
}

export async function activateHardwareCard(
	controller: PlaybackBankController,
	event: ReactMouseEvent<HTMLElement>,
	playback: PlaybackDefinition | null,
	slot: number,
) {
	if (!playback || isPlaybackControlTarget(event.target)) return;
	event.preventDefault();
	event.stopPropagation();
	if (
		isSetContextClick(event.nativeEvent) ||
		isPlaybackSetClickArmed(controller)
	) {
		openPlaybackConfiguration(controller, playback, slot);
		return;
	}
	if (controller.state.updateArmed) {
		if (playback.target.type === "cue_list")
			requestUpdateTarget(
				cueUpdateTarget(playback.target.cue_list_id, playback.number, null),
			);
		return;
	}
	await selectPlayback(controller, playback);
	if (playback.target.type === "cue_list") {
		controller.dispatch({ type: "OPEN_BUILTIN", kind: "cuelists" });
		controller.dispatch({
			type: "OPEN_BUILTIN_CUELIST",
			number: playback.number,
		});
	}
}

export async function assignPlayback(
	controller: PlaybackBankController,
	slot: number,
) {
	const target = controller.state.cueListSetTarget;
	if (target == null) return;
	const ok = await controller.command.execute(
		`SET ${target} AT ${controller.activePageNumber}.${slot}`,
	);
	if (!ok) return;
	await controller.server.refresh();
	controller.dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
}

export function requestPlaybackUpdate(
	playback: PlaybackDefinition | null,
	currentCue: Cue | null | undefined,
) {
	if (!playback || playback.target.type !== "cue_list") return;
	requestUpdateTarget(
		cueUpdateTarget(
			playback.target.cue_list_id,
			playback.number,
			currentCue?.id ? { id: currentCue.id, number: currentCue.number } : null,
		),
	);
}

export function isPlaybackControlTarget(target: EventTarget) {
	return (
		target instanceof Element &&
		Boolean(target.closest("button,input,.hardware-playback-controls"))
	);
}
