import type { ProgrammerPreloadLifecycleErrorKind } from "../features/programmerPreloadLifecycle/contracts";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";

export const PROGRAMMER_PRELOAD_LIFECYCLE_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly ProgrammerPreloadLifecycleErrorKind[];

export interface ProgrammerPreloadLifecycleErrorResponse {
	kind: ProgrammerPreloadLifecycleErrorKind;
	error: string;
	currentRevision: number | null;
	currentRelatedRevision: number | null;
	retryable: boolean;
}

export function decodeProgrammerPreloadLifecycleErrorResponse(
	value: unknown,
): ProgrammerPreloadLifecycleErrorResponse {
	const candidate = recordAt(value, "$");
	const response = exactRecordAt(value, "$", [
		"kind",
		"error",
		"retryable",
		...optionalKeys(candidate, [
			"current_revision",
			"current_related_revision",
		]),
	]);
	return {
		kind: enumAt(
			response.kind,
			"$.kind",
			PROGRAMMER_PRELOAD_LIFECYCLE_ERROR_KINDS,
		),
		error: stringAt(response.error, "$.error"),
		currentRevision: optionalInteger(response, "current_revision"),
		currentRelatedRevision: optionalInteger(
			response,
			"current_related_revision",
		),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function optionalKeys(
	value: Record<string, unknown>,
	keys: readonly string[],
) {
	return keys.filter((key) => key in value);
}

function optionalInteger(object: Record<string, unknown>, key: string) {
	return object[key] == null ? null : integerAt(object[key], `$.${key}`);
}
