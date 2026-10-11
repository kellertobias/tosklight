import type { PlaybackTopologyAction, PlaybackTopologyObject, PlaybackTopologyResolution } from "../features/playbackTopology/contracts";
import { sameKnownCueList, sameKnownPlayback } from "./playbackTopologyKnownBodies";
import type { CueList, PlaybackPage } from "./types";
import { WireValidationError } from "./wireValidation";

/** Rejects successful responses that escape the Page/Cuelist authority of an intent. */
export function validatePlaybackTopologyObjects(action: PlaybackTopologyAction, resolution: PlaybackTopologyResolution, objects: PlaybackTopologyObject[], status: "changed" | "no_change") {
	if (action.type === "save_cue_list") return validateSavedCueList(action, objects, status);
	if (action.type === "undo_cue_list" || action.type === "redo_cue_list") return validateCueListHistory(action, objects, status);
	const pageNumber = "address" in action ? action.address.page : action.number;
	if (objects.length !== 1) return invalid("only the authoritative requested Playback Page", objects);
	const object = objects[0];
	if (object.kind !== "playback_page") return invalid("the requested Playback Page", object);
	validateStorageId(object.objectId, action.expectedPageObjectId, String(pageNumber), "Playback Page");
	validateExactRevision(object.objectRevision, action.expectedPageRevision, status, "Playback Page");
	if (action.type === "delete_playback_page") {
		if (object.state !== "deleted") invalid("the deleted Playback Page", object);
		return;
	}
	if (object.state !== "present") return invalid("the present Playback Page", object);
	const page = object.body as PlaybackPage;
	if (page.number !== pageNumber) return invalid("the requested Playback Page number", page.number);
	if ("number" in action) {
		if (resolution.kind !== "page" || page.name !== action.name) invalid("the requested Playback Page name", page);
		if (action.type === "create_playback_page" && Object.keys(page.assignments).length)
			invalid("an empty newly created Playback Page", page.assignments);
		return;
	}
	if (resolution.kind !== "playback" || resolution.page !== action.address.page || resolution.position !== action.address.position)
		return invalid("the requested Playback address", resolution);
	const definition = page.assignments[String(action.address.position)];
	if (action.type === "clear_playback") {
		if (definition) invalid("the cleared Playback position", definition);
	} else if (action.type === "assign_group_master") {
		if (definition?.target.type !== "group" || definition.target.group_id !== action.groupObjectId)
			invalid("the explicitly requested Group Master", definition);
	} else if (!definition || !sameKnownPlayback(definition, action.definition)) {
		invalid("the submitted Playback definition", definition);
	}
}

function validateSavedCueList(
	action: Extract<PlaybackTopologyAction, { type: "save_cue_list" }>,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (objects.length !== 1)
		return invalid("only the authoritative requested Cuelist", objects);
	const object = objects[0];
	if (
		object.kind !== "cue_list" ||
		object.state !== "present" ||
		(object.body as { id: string }).id !== action.cueListId
	)
		invalid("the authoritative requested Cuelist", objects);
	validateStorageId(
		object.objectId,
		action.expectedObjectId,
		action.cueListId,
		"Cuelist",
	);
	validateExactRevision(
		object.objectRevision,
		action.expectedRevision,
		status,
		"Cuelist",
	);
	if (!sameKnownCueList(object.body as CueList, action.body))
		invalid("the submitted Cuelist known fields", objects);
}

function validateCueListHistory(
	action: Extract<
		PlaybackTopologyAction,
		{ type: "undo_cue_list" | "redo_cue_list" }
	>,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (status !== "changed" || objects.length !== 1)
		return invalid("one changed authoritative Cuelist", objects);
	const object = objects[0];
	if (
		object.kind !== "cue_list" ||
		object.state !== "present" ||
		(object.body as { id: string }).id !== action.cueListId ||
		object.objectId !== action.expectedObjectId
	)
		return invalid("the authoritative requested Cuelist", objects);
	validateExactRevision(
		object.objectRevision,
		action.expectedRevision,
		status,
		"Cuelist",
	);
}

function validateExactRevision(
	actual: number,
	expected: number,
	status: "changed" | "no_change",
	label: string,
) {
	if (status === "changed") validateChangedRevision(actual, expected, label);
	else validateNoChangeRevision(actual, expected, label);
}

function validateChangedRevision(
	actual: number,
	expected: number,
	label: string,
) {
	if (actual !== expected + 1)
		invalid(`${label} revision ${expected + 1}`, actual);
}

function validateNoChangeRevision(
	actual: number,
	expected: number,
	label: string,
) {
	if (actual !== expected) invalid(`${label} revision ${expected}`, actual);
}

function validateStorageId(
	actual: string,
	expected: string | null,
	fallback: string,
	label: string,
) {
	if (actual !== (expected ?? fallback))
		invalid(`${label} storage identity ${expected ?? fallback}`, actual);
}

function invalid(expected: string, actual: unknown): never {
	throw new WireValidationError("$.objects", expected, actual);
}
