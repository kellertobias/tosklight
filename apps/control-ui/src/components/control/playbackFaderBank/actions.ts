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
	const { dispatch, activePageNumber, buttons, topology } = controller;
	if (activePageNumber == null) return;
	const slotData = controller.slots.find((candidate) => candidate.slot === slot);
	const fallbackButtons = Math.max(
		0,
		Math.min(3, slotData?.row?.button_count ?? buttons ?? 3),
	);
	const playbackObject = playback
		? topology.playbacks.find(
				(candidate) => candidate.body.number === playback.number,
			)
		: null;
	controller.setConfiguration({
		playback: normalizePlaybackTopology(
			playback ??
				emptyConfiguration(
					activePageNumber,
					slot,
					fallbackButtons,
					slotData?.row?.has_fader ?? true,
					topology.cueLists[0]?.body.id ?? "",
				),
			fallbackButtons,
			slotData?.row?.has_fader ?? true,
		),
		page: activePageNumber,
		slot,
		empty: !playback,
		fallbackButtons,
		expectedPageRevision: controller.pageObject?.revision ?? 0,
		expectedPageObjectId: controller.pageObject?.id ?? null,
		expectedPlaybackRevision: playbackObject?.revision ?? 0,
		expectedPlaybackObjectId: playbackObject?.id ?? null,
	});
	dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
	dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	dispatch({ type: "SET_SHIFT_ARMED", value: false });
}

export async function selectPlayback(
	controller: PlaybackBankController,
	playback: PlaybackDefinition,
) {
	const outcome = await controller.runtimeActions?.poolPlaybackAction(
		playback.number,
		"select",
		{ surface: "physical" },
	);
	if (!outcome) return false;
	if (controller.selectionPending) void controller.commandLineActions?.reset();
	return true;
}

export async function recordPlayback(
	controller: PlaybackBankController,
	event: ReactMouseEvent,
	playback: PlaybackDefinition | null,
	slot: number,
) {
	if (!controller.state.storeArmed || controller.activePageNumber == null) return;
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
	await controller.commandLineActions?.reset();
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
	if (!(await selectPlayback(controller, playback))) return;
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
	if (target == null || controller.activePageNumber == null) return;
	const source = controller.topology.playbacks.find(
		(candidate) =>
			candidate.body.number === target &&
			candidate.body.target.type === "cue_list",
	);
	if (!source) return;
	const outcome = await controller.topologyActions?.mapExistingPlayback(
		controller.activePageNumber,
		slot,
		target,
		{
			expectedPageRevision: controller.pageObject?.revision ?? 0,
			expectedPageObjectId: controller.pageObject?.id ?? null,
			expectedPlaybackRevision: source.revision,
			expectedPlaybackObjectId: source.id,
		},
	);
	if (!outcome) return;
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
