import type {
	PresetRecordingOutcome,
	PresetRecordingRequest,
} from "../features/presetRecording/contracts";
import type { ShowObject } from "../features/showObjects/contracts";
import {
	normalizePresetFamily,
	PRESET_FAMILIES,
	PRESET_FAMILY_TYPE,
	presetStorageKey,
	type PresetAddress,
	type PresetFamily,
} from "../presetFamilies";
import type {
	PresetRecordErrorKind as WirePresetRecordErrorKind,
	PresetRecordRequest as WirePresetRecordRequest,
	PresetRecordingFamily as WirePresetRecordingFamily,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

export const PRESET_RECORD_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WirePresetRecordErrorKind[];

export type PresetRecordErrorKind = WirePresetRecordErrorKind;

const WIRE_FAMILY: Record<PresetFamily, WirePresetRecordingFamily> = {
	Mixed: "mixed",
	Intensity: "intensity",
	Color: "color",
	Position: "position",
	Beam: "beam",
};

export interface PresetRecordErrorResponse {
	kind: PresetRecordErrorKind;
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}

export function encodePresetRecordingRequest(request: PresetRecordingRequest) {
	validateRequest(request);
	return {
		request_id: request.requestId,
		address: {
			family: WIRE_FAMILY[request.address.family],
			number: request.address.number,
		},
		name: request.name,
		mode: request.mode,
		expected_object_revision: request.expectedObjectRevision,
	} satisfies WirePresetRecordRequest;
}

export function decodePresetRecordingOutcome(
	value: unknown,
	expectedRequest: PresetRecordingRequest,
): PresetRecordingOutcome {
	const response = recordAt(value, "$");
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	const expectedFields = [
		"request_id",
		"correlation_id",
		"replayed",
		"status",
		"show_revision",
		"preset",
	];
	if (status === "changed") expectedFields.push("event_sequence");
	exactRecordAt(response, "$", expectedFields);
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== expectedRequest.requestId)
		throw new WireValidationError(
			"$.request_id",
			`request ${expectedRequest.requestId}`,
			requestId,
		);
	const preset = decodeRecordedPreset(response.preset, expectedRequest);
	validateOutcomeRevision(status, preset.revision, expectedRequest);
	const base = {
		requestId,
		correlationId: uuidAt(response.correlation_id, "$.correlation_id"),
		replayed: booleanAt(response.replayed, "$.replayed"),
		status,
		showRevision: integerAt(response.show_revision, "$.show_revision"),
		preset,
	};
	return status === "changed"
		? {
				...base,
				status,
				eventSequence: integerAt(
					response.event_sequence,
					"$.event_sequence",
				),
			}
		: { ...base, status };
}

function validateOutcomeRevision(
	status: "changed" | "no_change",
	revision: number,
	request: PresetRecordingRequest,
) {
	const expected =
		status === "changed"
			? request.expectedObjectRevision + 1
			: request.expectedObjectRevision;
	if (!Number.isSafeInteger(expected) || revision !== expected)
		throw new WireValidationError(
			"$.preset.revision",
			`${status} revision ${expected}`,
			revision,
		);
}

export function decodePresetRecordErrorResponse(
	value: unknown,
): PresetRecordErrorResponse {
	const response = exactRecordAt(value, "$", [
		"kind",
		"error",
		"current_revision",
		"retryable",
	]);
	return {
		kind: enumAt(response.kind, "$.kind", PRESET_RECORD_ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision:
			response.current_revision == null
				? null
				: integerAt(response.current_revision, "$.current_revision"),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function decodeRecordedPreset(
	value: unknown,
	expectedRequest: PresetRecordingRequest,
): ShowObject<"preset"> {
	const preset = exactRecordAt(value, "$.preset", ["id", "revision", "body"]);
	const id = validatePrintable(preset.id, "$.preset.id", 256, "Preset ID");
	validatePresetId(id, expectedRequest.address);
	return {
		kind: "preset",
		id,
		revision: integerAt(preset.revision, "$.preset.revision"),
		updated_at: "",
		body: decodePresetBody(preset.body, expectedRequest),
	};
}

function decodePresetBody(
	value: unknown,
	expectedRequest: PresetRecordingRequest,
): ShowObject<"preset">["body"] {
	const body = recordAt(value, "$.preset.body");
	const family = familyAt(body.family, "$.preset.body.family");
	if (normalizePresetFamily(family) !== expectedRequest.address.family)
		throw new WireValidationError(
			"$.preset.body.family",
			`family ${expectedRequest.address.family}`,
			family,
		);
	const name = validatePrintable(
		body.name,
		"$.preset.body.name",
		256,
		"Preset name",
	);
	if (name !== expectedRequest.name)
		throw new WireValidationError(
			"$.preset.body.name",
			`name ${expectedRequest.name}`,
			name,
		);
	const number = integerAt(body.number, "$.preset.body.number");
	if (number !== expectedRequest.address.number)
		throw new WireValidationError(
			"$.preset.body.number",
			`number ${expectedRequest.address.number}`,
			number,
		);
	valuesAt(body.values, "$.preset.body.values");
	if ("group_values" in body)
		valuesAt(body.group_values, "$.preset.body.group_values");
	return { ...body, family } as ShowObject<"preset">["body"];
}

function validateRequest(request: PresetRecordingRequest) {
	validatePrintable(request.requestId, "$.requestId", 128, "request ID");
	validatePrintable(request.name, "$.name", 256, "Preset name");
	enumAt(request.address.family, "$.address.family", PRESET_FAMILIES);
	const number = integerAt(request.address.number, "$.address.number");
	if (number < 1 || number > 4_294_967_295)
		throw new WireValidationError(
			"$.address.number",
			"positive 32-bit integer",
			request.address.number,
		);
	enumAt(request.mode, "$.mode", ["merge", "overwrite"]);
	integerAt(request.expectedObjectRevision, "$.expectedObjectRevision");
}

function validatePresetId(id: string, address: PresetAddress) {
	if (id === presetStorageKey(address)) return;
	const plain = /^(\d+)$/.exec(id);
	if (plain && Number(plain[1]) === address.number) return;
	const dotted = /^(\d+)\.(\d+)$/.exec(id);
	if (
		dotted &&
		Number(dotted[1]) === PRESET_FAMILY_TYPE[address.family] &&
		Number(dotted[2]) === address.number
	)
		return;
	throw new WireValidationError(
		"$.preset.id",
		`storage identity for ${presetStorageKey(address)}`,
		id,
	);
}

function familyAt(value: unknown, path: string): PresetFamily | "All" {
	return enumAt(value, path, [...PRESET_FAMILIES, "All"]);
}

function valuesAt(value: unknown, path: string) {
	const values = recordAt(value, path);
	for (const [owner, attributes] of Object.entries(values))
		recordAt(attributes, `${path}.${owner}`);
}

function validatePrintable(
	value: unknown,
	path: string,
	byteLimit: number,
	label: string,
) {
	if (
		typeof value !== "string" ||
		!value.trim() ||
		new TextEncoder().encode(value).length > byteLimit ||
		/\p{Cc}/u.test(value)
	)
		throw new WireValidationError(path, `1-${byteLimit} printable ${label} bytes`, value);
	return value;
}

function uuidAt(value: unknown, path: string) {
	const decoded = stringAt(value, path);
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(decoded))
		throw new WireValidationError(path, "hyphenated UUID", value);
	return decoded;
}
