import type { SpeedGroupErrorKind } from "../features/speedGroupRuntime/contracts";
import type { SpeedGroupErrorKind as WireSpeedGroupErrorKind } from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";

const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WireSpeedGroupErrorKind[];

export interface SpeedGroupErrorResponse {
	kind: SpeedGroupErrorKind;
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}

export function decodeSpeedGroupErrorResponse(
	value: unknown,
): SpeedGroupErrorResponse {
	const response = recordAt(value, "$"),
		fields = ["kind", "error", "current_revision", "retryable"].filter(
			(field) => field in response,
		);
	exactRecordAt(response, "$", fields);
	return {
		kind: enumAt(response.kind, "$.kind", ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision:
			response.current_revision == null
				? null
				: integerAt(response.current_revision, "$.current_revision"),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}
