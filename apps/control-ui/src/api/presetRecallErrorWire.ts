import type { PresetRecallErrorKind as WirePresetRecallErrorKind } from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";

export const PRESET_RECALL_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WirePresetRecallErrorKind[];

export type PresetRecallErrorKind = WirePresetRecallErrorKind;

export interface PresetRecallErrorResponse {
	kind: PresetRecallErrorKind;
	error: string;
	currentRevision: number | null;
	currentRelatedRevision: number | null;
	retryable: boolean;
}

export function decodePresetRecallErrorResponse(
	value: unknown,
): PresetRecallErrorResponse {
	const response = exactRecordAt(recordAt(value, "$"), "$", [
		"kind",
		"error",
		"retryable",
		"current_revision",
		"current_related_revision",
	]);
	return {
		kind: enumAt(response.kind, "$.kind", PRESET_RECALL_ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision: optionalInteger(response, "current_revision"),
		currentRelatedRevision: optionalInteger(
			response,
			"current_related_revision",
		),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function optionalInteger(object: Record<string, unknown>, key: string) {
	return object[key] == null ? null : integerAt(object[key], `$.${key}`);
}
