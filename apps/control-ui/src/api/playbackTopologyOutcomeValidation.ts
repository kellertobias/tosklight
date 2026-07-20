import type { CueList, PlaybackDefinition, PlaybackPage } from "./types";
import type {
	PlaybackTopologyAction,
	PlaybackTopologyObject,
	PlaybackTopologyResolution,
} from "../features/playbackTopology/contracts";
import { WireValidationError } from "./wireValidation";
import {
	sameKnownCueList,
	sameKnownPlayback,
} from "./playbackTopologyKnownBodies";

/** Limits a successful response to authority owned by the submitted action. */
export function validatePlaybackTopologyObjects(
	action: PlaybackTopologyAction,
	resolution: PlaybackTopologyResolution,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (action.type === "save_cue_list")
		return validateSavedCueList(action, objects, status);
	if (resolution.kind !== "page_slot")
		return invalid("page-slot resolution", resolution);
	if (action.type === "configure_slot")
		return validateConfiguredSlot(
			action,
			resolution.playbackNumber,
			objects,
			status,
		);
	validateClearedSlot(action, resolution.playbackNumber, objects, status);
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

function validateConfiguredSlot(
	action: Extract<PlaybackTopologyAction, { type: "configure_slot" }>,
	playbackNumber: number | null,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (playbackNumber == null)
		return invalid("a configured Playback number", playbackNumber);
	if (objects.length !== 2)
		return invalid("only the configured Page and Playback", objects);
	const page = matchingPage(objects, action.page);
	const playback = objects.find(
		(object) =>
			object.kind === "playback" &&
			object.state === "present" &&
			(object.body as PlaybackDefinition).number === playbackNumber,
	);
	if (
		!page ||
		(page.body as PlaybackPage).slots[String(action.slot)] !== playbackNumber
	)
		invalid("the configured Page mapping", objects);
	if (!playback) invalid("the configured Playback", objects);
	const presentPlayback = playback as Extract<
		PlaybackTopologyObject,
		{ state: "present" }
	>;
	validateStorageId(
		page.objectId,
		action.expectedPageObjectId,
		String(action.page),
		"Playback Page",
	);
	validateStorageId(
		presentPlayback.objectId,
		action.expectedPlaybackObjectId,
		String(playbackNumber),
		"Playback",
	);
	validateConfigureRevisions(action, page, presentPlayback, status);
	if (
		!sameKnownPlayback(
			presentPlayback.body as PlaybackDefinition,
			action.playback,
			playbackNumber,
		)
	)
		invalid("the server-normalized requested Playback", objects);
}

function validateClearedSlot(
	action: Extract<PlaybackTopologyAction, { type: "clear_mapped_playback" }>,
	playbackNumber: number | null,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (playbackNumber == null) return validateEmptyClear(action, objects, status);
	if (status !== "changed") invalid("a changed mapped-Playback clear", objects);
	const deleted = objects.filter(
		(object) => object.kind === "playback" && object.state === "deleted",
	);
	const pages = objects.filter(
		(object) => object.kind === "playback_page" && object.state === "present",
	);
	if (
		action.expectedPlaybackObjectId === null ||
		deleted.length !== 1 ||
		deleted[0].objectId !== action.expectedPlaybackObjectId ||
		deleted[0].objectRevision !== action.expectedPlaybackRevision + 1
	)
		invalid("the exact deleted mapped Playback", objects);
	if (pages.length + deleted.length !== objects.length || pages.length === 0)
		invalid("only cleared Pages and the mapped Playback", objects);
	if (!matchingPage(pages, action.page))
		invalid("the authoritative cleared Page", objects);
	const requestedPage = matchingPage(pages, action.page);
	if (!requestedPage) invalid("the authoritative cleared Page", objects);
	validateStorageId(
		requestedPage.objectId,
		action.expectedPageObjectId,
		String(action.page),
		"Playback Page",
	);
	validateChangedRevision(
		requestedPage.objectRevision,
		action.expectedPageRevision,
		"Playback Page",
	);
	for (const page of pages) {
		if (page.state !== "present")
			return invalid("only present cleared Pages", objects);
		const slots = (page.body as PlaybackPage).slots;
		if (Object.values(slots).includes(playbackNumber))
			invalid("Pages without the deleted Playback mapping", objects);
	}
}

function validateEmptyClear(
	action: Extract<PlaybackTopologyAction, { type: "clear_mapped_playback" }>,
	objects: PlaybackTopologyObject[],
	status: "changed" | "no_change",
) {
	if (status !== "no_change") invalid("a no-change empty-slot clear", objects);
	if (action.expectedPlaybackObjectId !== null || objects.length > 1)
		return invalid("at most the requested empty Page", objects);
	if (objects.length === 0) return;
	const page = matchingPage(objects, action.page);
	if (
		!page ||
		(page.body as PlaybackPage).slots[String(action.slot)] != null
	)
		invalid("the requested empty Page", objects);
	validateStorageId(
		page.objectId,
		action.expectedPageObjectId,
		String(action.page),
		"Playback Page",
	);
	validateNoChangeRevision(
		page.objectRevision,
		action.expectedPageRevision,
		"Playback Page",
	);
}

function validateConfigureRevisions(
	action: Extract<PlaybackTopologyAction, { type: "configure_slot" }>,
	page: Extract<PlaybackTopologyObject, { state: "present" }>,
	playback: Extract<PlaybackTopologyObject, { state: "present" }>,
	status: "changed" | "no_change",
) {
	const pageChanged = validatePossibleRevision(
		page.objectRevision,
		action.expectedPageRevision,
		status,
		"Playback Page",
	);
	const playbackChanged = validatePossibleRevision(
		playback.objectRevision,
		action.expectedPlaybackRevision,
		status,
		"Playback",
	);
	if (status === "changed" && !pageChanged && !playbackChanged)
		invalid("at least one changed configured object revision", [page, playback]);
}

function validatePossibleRevision(
	actual: number,
	expected: number,
	status: "changed" | "no_change",
	label: string,
) {
	if (status === "no_change") {
		validateNoChangeRevision(actual, expected, label);
		return false;
	}
	if (actual !== expected && actual !== expected + 1)
		invalid(`${label} revision ${expected} or ${expected + 1}`, actual);
	return actual === expected + 1;
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

function validateChangedRevision(actual: number, expected: number, label: string) {
	if (actual !== expected + 1)
		invalid(`${label} revision ${expected + 1}`, actual);
}

function validateNoChangeRevision(actual: number, expected: number, label: string) {
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

function matchingPage(objects: PlaybackTopologyObject[], pageNumber: number) {
	return objects.find(
		(object) =>
			object.kind === "playback_page" &&
			object.state === "present" &&
			(object.body as PlaybackPage).number === pageNumber,
	) as Extract<PlaybackTopologyObject, { state: "present" }> | undefined;
}

function invalid(expected: string, actual: unknown): never {
	throw new WireValidationError("$.objects", expected, actual);
}
