import type { OutputRuntimeErrorKind } from "../features/outputRuntime/contracts";
import type { OutputRuntimeErrorKind as WireOutputErrorKind } from "./generated/light-wire";
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
] as const satisfies readonly WireOutputErrorKind[];

export interface OutputRuntimeErrorResponse {
	kind: OutputRuntimeErrorKind;
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}

export function decodeOutputRuntimeErrorResponse(
	value: unknown,
): OutputRuntimeErrorResponse {
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
