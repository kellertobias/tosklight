import type {
	ShowObjectBodies,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import { decodeRecordedGroupBody } from "./groupRecordingBodyWire";
import { integerAt, recordAt } from "./playbackWirePrimitives";
import { decodeCueListBody } from "./showObjectCueWire";
import {
	decodePlaybackBody,
	decodePlaybackPageBody,
} from "./showObjectPlaybackWire";
import { WireValidationError } from "./wireValidation";

export function decodeShowObjectBody<K extends ShowObjectKind>(
	kind: K,
	value: unknown,
	path: string,
	objectId?: string,
): ShowObjectBodies[K] {
	const body =
		kind === "group"
			? decodeRecordedGroupBody(value, objectId ?? "")
			: kind === "preset"
				? decodePreset(value, path)
				: kind === "cue_list"
					? decodeCueListBody(value, path, objectId)
					: kind === "playback"
						? decodePlaybackBody(value, path, objectId)
						: decodePlaybackPageBody(value, path, objectId);
	return body as ShowObjectBodies[K];
}

function decodePreset(value: unknown, path: string) {
	const body = recordAt(value, path);
	plainStringAt(body.name, `${path}.name`);
	integerAt(body.number, `${path}.number`);
	recordAt(body.values, `${path}.values`);
	return body;
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string")
		throw new WireValidationError(path, "string", value);
	return value;
}
