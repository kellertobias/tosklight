import type { MouseEvent as ReactMouseEvent } from "react";
import type { Cue, CueList, PlaybackDefinition } from "../../../api/types";
import {
	resolveRecordOption,
	touchCueRecordPlan,
} from "../../../features/recordUpdateOptions/options";
import { loadRecordSettings } from "../../setup/ProgrammerDefaults";
import {
	normalizePlaybackTopology,
	withFunctionDefaults,
} from "../PlaybackConfigurationModal";
import { cueUpdateTarget, requestUpdateTarget } from "../updateWorkflow";
import type { AssignedPlayback } from "./types";
import type { PlaybackBankController } from "./controller";
import { emptyConfiguration } from "./feedback";

export function isPlaybackSetClickArmed(controller: PlaybackBankController) {
	const { state } = controller;
	return (
		controller.setInteractionArmed ||
		state.playbackSetArmed ||
		(state.cueListSetArmed && state.cueListSetTarget == null)
	);
}

export function openPlaybackConfiguration(
	controller: PlaybackBankController,
	playback: AssignedPlayback | null,
	slot: number,
) {
	const { dispatch, activePageNumber, buttons, topology } = controller;
	if (activePageNumber == null) return;
	const slotData = controller.slots.find(
		(candidate) => candidate.slot === slot,
	);
	const fallbackButtons = Math.max(
		0,
		Math.min(3, slotData?.row?.button_count ?? buttons ?? 3),
	);
	controller.setConfiguration({
		playback: normalizePlaybackTopology(
			playback ??
				emptyConfiguration(
					activePageNumber,
					slot,
					fallbackButtons,
					slotData?.row?.presentation !== "buttons",
					topology.cueLists[0]?.body.id ?? "",
				),
			fallbackButtons,
			slotData?.row?.presentation !== "buttons",
		),
		page: activePageNumber,
		slot,
		empty: !playback,
		fallbackButtons,
		expectedPageRevision: controller.pageObject?.revision ?? 0,
		expectedPageObjectId: controller.pageObject?.id ?? null,
	});
	dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
	dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	dispatch({ type: "SET_SHIFT_ARMED", value: false });
}

export async function selectPlayback(
	controller: PlaybackBankController,
	playback: AssignedPlayback,
) {
	const outcome = await controller.runtimeActions?.playbackAction(
		playback.address,
		"select",
		{control_source: {surface_id: controller.surfaceId, control_id: playback.address.position * 10}},
	);
	if (!outcome) return false;
	if (controller.selectionPending) void controller.commandLineActions?.reset();
	return true;
}

export async function offPlayback(
	controller: PlaybackBankController,
	playback: AssignedPlayback,
) {
	if (!controller.offPending) return false;
	const outcome = await controller.runtimeActions?.playbackAction(
		playback.address,
		"off",
		{control_source: {surface_id: controller.surfaceId, control_id: playback.address.position * 10}},
	);
	if (!outcome) return false;
	await controller.commandLineActions?.reset();
	return true;
}

export async function assignDynamicPlayback(
	controller: PlaybackBankController,
	slot: number,
) {
	const command = controller.commandLineActions;
	if (!command || !controller.dynamicAssignmentPending) return false;
	const prefix = controller.commandLine?.text.trim();
	if (!prefix) return false;
	const address =
		controller.playbackAddressing === "explicit_page"
			? `${controller.activePageNumber} . ${slot}`
			: String(slot);
	const outcome = await command.execute(`${prefix} AT PBK ${address}`);
	return outcome.executed;
}

export async function assignGroupPlayback(
	controller: PlaybackBankController,
	slot: number,
) {
	if (controller.activePageNumber == null) return false;
	const slotData = controller.slots.find(
		(candidate) => candidate.slot === slot,
	);
	const existing = slotData?.playback ?? null;
	if (!controller.setInteraction) {
		const match = controller.commandLine?.text
			.trim()
			.match(/^ASSIGN\s+GROUP\s+(\S+)$/i);
		if (!match) return false;
		const groupId = match[1];
		const fallbackButtons = Math.max(
			0,
			Math.min(3, slotData?.row?.button_count ?? controller.buttons ?? 3),
		);
		const draft = withFunctionDefaults(
			existing ??
				emptyConfiguration(
					controller.activePageNumber,
					slot,
					fallbackButtons,
					slotData?.row?.presentation !== "buttons",
					"",
				),
			"group",
			"",
			groupId,
		);
		const configure = existing ? controller.topologyActions?.updatePlayback : controller.topologyActions?.assignPlayback;
		const outcome = await configure?.({page: controller.activePageNumber, position: slot}, {...draft, name: `Group ${groupId}`}, {
			expectedPageRevision: controller.pageObject?.revision ?? 0,
			expectedPageObjectId: controller.pageObject?.id ?? null,
		});
		if (!outcome) return false;
		await controller.commandLineActions?.reset();
		return true;
	}

	const intent = await controller.setInteraction.choosePlayback(
		{
			addressing: controller.playbackAddressing,
			pageNumber: controller.activePageNumber,
			slot,
			pageObjectId: controller.pageObject?.id ?? null,
			pageObjectRevision: controller.pageObject?.revision ?? 0,
		},
		controller.hardware ? "hardware" : "touch",
	);
	return (
		intent?.type === "open_playback_settings" ||
		intent?.type === "assign_group_master"
	);
}

export async function recordPlayback(
	controller: PlaybackBankController,
	event: ReactMouseEvent,
	_playback: AssignedPlayback | null,
	cueList: CueList | null | undefined,
	slot: number,
) {
	if (!controller.state.storeArmed || controller.activePageNumber == null)
		return;
	event.preventDefault();
	event.stopPropagation();
	const page = controller.activePageNumber;
	const settings = loadRecordSettings();
	const option = await resolveRecordOption(
		controller.commandLine?.text ?? "",
		controller.programmingUpdate,
	);
	const plan = await touchCueRecordPlan(
		option,
		cueList?.cues.map((cue) => cue.number) ?? [],
		controller.chooseCueRecordOperation,
	);
	if (!plan) return;
	const outcome = await controller.cueRecording?.record({
		target: {kind: "playback", page, position: slot},
		operation: plan.operation,
		...(plan.cueNumber ? { cueNumber: plan.cueNumber } : {}),
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
	playback: AssignedPlayback | null,
	slot: number,
) {
	if (controller.groupAssignmentPending || controller.setInteractionArmed) {
		event.preventDefault();
		event.stopPropagation();
		await assignGroupPlayback(controller, slot);
		return;
	}
	if (!playback || isPlaybackControlTarget(event.target)) return;
	event.preventDefault();
	event.stopPropagation();
	if (controller.dynamicAssignmentPending) {
		await assignDynamicPlayback(controller, slot);
		return;
	}
	if (isPlaybackSetClickArmed(controller)) {
		await assignGroupPlayback(controller, slot);
		return;
	}
	if (controller.state.updateArmed) {
		if (playback.target.type === "cue_list")
			requestUpdateTarget(
				cueUpdateTarget(playback.target.cue_list_id, playback.address, null),
			);
		return;
	}
	if (!(await selectPlayback(controller, playback))) return;
	if (playback.target.type === "cue_list") {
		controller.dispatch({ type: "OPEN_BUILTIN", kind: "cuelists" });
		controller.dispatch({
			type: "OPEN_BUILTIN_CUELIST",
			number: controller.topology.cueLists.find(object => object.body.id === (playback.target.type === "cue_list" ? playback.target.cue_list_id : ""))?.body.pool_number ?? 1,
		});
	}
}

export async function assignPlayback(
	controller: PlaybackBankController,
	slot: number,
) {
	const target = controller.state.cueListSetTarget;
	if (target == null || controller.activePageNumber == null) return;
	const source = controller.topology.cueLists.find(candidate => candidate.body.pool_number === target);
	if (!source) return;
	const draft = emptyConfiguration(controller.activePageNumber, slot, controller.buttons ?? 3, true, source.body.id);
	const outcome = await controller.topologyActions?.assignPlayback({page: controller.activePageNumber, position: slot}, draft, {
		expectedPageRevision: controller.pageObject?.revision ?? 0,
		expectedPageObjectId: controller.pageObject?.id ?? null,
	});
	if (!outcome) return;
	controller.dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
}

export function requestPlaybackUpdate(
	playback: AssignedPlayback | null,
	currentCue: Cue | null | undefined,
) {
	if (!playback || playback.target.type !== "cue_list") return;
	requestUpdateTarget(
		cueUpdateTarget(
			playback.target.cue_list_id,
			playback.address,
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
