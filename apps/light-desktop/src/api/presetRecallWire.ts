import type {
	PresetRecallOutcome,
	PresetRecallRequest,
} from "../features/presetRecall/contracts";
import type { ShowObject } from "../features/showObjects/contracts";
import {
	normalizePresetFamily,
	PRESET_FAMILIES,
	type PresetFamily,
	presetStorageKey,
} from "../presetFamilies";
import type {
	PresetRecallRequest as WirePresetRecallRequest,
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
import { decodeProgrammerValuesProjection } from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export {
	decodePresetRecallErrorResponse,
	PRESET_RECALL_ERROR_KINDS,
	type PresetRecallErrorKind,
	type PresetRecallErrorResponse,
} from "./presetRecallErrorWire";

const WIRE_FAMILY: Record<PresetFamily, WirePresetRecordingFamily> = {
	Mixed: "mixed",
	Intensity: "intensity",
	Color: "color",
	Position: "position",
	Beam: "beam",
};

export function encodePresetRecallRequest(
	request: PresetRecallRequest,
): WirePresetRecallRequest {
	validateRequest(request);
	return {
		request_id: request.requestId,
		address: {
			family: WIRE_FAMILY[request.address.family],
			number: request.address.number,
		},
		expected_preset_revision: request.expectedPresetRevision,
		expected_show_revision: request.expectedShowRevision,
		expected_programmer_revision: request.expectedProgrammerRevision,
		expected_capture_mode_revision: request.expectedCaptureModeRevision,
		expected_selection_revision: request.expectedSelectionRevision,
	};
}

export function decodePresetRecallOutcome(
	value: unknown,
	expectedUserId: string,
	expectedRequest: PresetRecallRequest,
): PresetRecallOutcome {
	const response = recordAt(value, "$");
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	assertOutcomeFields(response);
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== expectedRequest.requestId)
		throw mismatch("$.request_id", expectedRequest.requestId, requestId);
	const projection = optionalProjection(
		response,
		expectedUserId,
		expectedRequest,
		status,
	);
	const eventSequence = optionalInteger(response, "event_sequence", "$");
	assertValuesPair(status, projection, eventSequence, expectedRequest);
	const interactionEventSequence = optionalInteger(
		response,
		"interaction_event_sequence",
		"$",
	);
	const selectionRevision = integerAt(
		response.selection_revision,
		"$.selection_revision",
	);
	assertSelectionOutcome(
		selectionRevision,
		interactionEventSequence,
		expectedRequest,
	);
	const base = {
		requestId,
		correlationId: uuidAt(response.correlation_id, "$.correlation_id"),
		replayed: booleanAt(response.replayed, "$.replayed"),
		showRevision: exactRevision(
			response.show_revision,
			"$.show_revision",
			expectedRequest.expectedShowRevision,
		),
		programmerRevision: programmerRevision(
			response.programmer_revision,
			projection,
			expectedRequest,
		),
		captureModeRevision: exactRevision(
			response.capture_mode_revision,
			"$.capture_mode_revision",
			expectedRequest.expectedCaptureModeRevision,
		),
		selectionRevision,
		interactionEventSequence,
		appliedFixtures: exactRevision(
			response.applied_fixtures,
			"$.applied_fixtures",
			expectedRequest.selectedFixtureCount,
		),
		activeContext: activeContextAt(response.active_context, expectedRequest),
		preset: decodeRecalledPreset(response.preset, expectedRequest),
		warning: optionalString(response, "warning", "$"),
	};
	return status === "changed"
		? { ...base, status, projection, eventSequence }
		: { ...base, status, projection: null, eventSequence: null };
}

function optionalProjection(
	response: Record<string, unknown>,
	expectedUserId: string,
	request: PresetRecallRequest,
	status: "changed" | "no_change",
) {
	if (response.projection == null) return null;
	if (status === "no_change")
		throw new WireValidationError(
			"$.projection",
			"absent for no_change",
			response.projection,
		);
	const projection = decodeProgrammerValuesProjection(
		response.projection,
		"$.projection",
		expectedUserId,
	);
	if (projection.revision !== request.expectedProgrammerRevision + 1)
		throw mismatch(
			"$.projection.revision",
			request.expectedProgrammerRevision + 1,
			projection.revision,
		);
	return projection;
}

function assertValuesPair(
	status: "changed" | "no_change",
	projection: PresetRecallOutcome["projection"],
	eventSequence: number | null,
	request: PresetRecallRequest,
) {
	if ((projection === null) !== (eventSequence === null))
		throw new WireValidationError(
			"$",
			"a paired values projection and event sequence",
			{ projection, eventSequence },
		);
	if (status === "no_change" && (projection !== null || eventSequence !== null))
		throw new WireValidationError(
			"$",
			"a sparse no_change outcome",
			request.requestId,
		);
}

function assertSelectionOutcome(
	revision: number,
	sequence: number | null,
	request: PresetRecallRequest,
) {
	const valid =
		sequence == null
			? revision === request.expectedSelectionRevision
			: revision > request.expectedSelectionRevision;
	if (!valid)
		throw mismatch(
			"$.selection_revision",
			sequence == null
				? request.expectedSelectionRevision
				: `greater than ${request.expectedSelectionRevision}`,
			revision,
		);
}

function programmerRevision(
	value: unknown,
	projection: PresetRecallOutcome["projection"],
	request: PresetRecallRequest,
) {
	const revision = integerAt(value, "$.programmer_revision");
	const expected = projection?.revision ?? request.expectedProgrammerRevision;
	if (revision !== expected)
		throw mismatch("$.programmer_revision", expected, revision);
	return revision;
}

function decodeRecalledPreset(
	value: unknown,
	request: PresetRecallRequest,
): ShowObject<"preset"> {
	const preset = exactRecordAt(value, "$.preset", ["id", "revision", "body"]);
	const id = printableAt(preset.id, "$.preset.id", 256);
	if (id !== request.presetId)
		throw mismatch("$.preset.id", request.presetId, id);
	const body = decodePresetBody(preset.body, request);
	return {
		kind: "preset",
		id,
		revision: exactRevision(
			preset.revision,
			"$.preset.revision",
			request.expectedPresetRevision,
		),
		updated_at: "",
		body,
	};
}

function decodePresetBody(
	value: unknown,
	request: PresetRecallRequest,
): ShowObject<"preset">["body"] {
	const body = recordAt(value, "$.preset.body");
	stringAt(body.name, "$.preset.body.name");
	const number = integerAt(body.number, "$.preset.body.number");
	if (number !== request.address.number)
		throw mismatch("$.preset.body.number", request.address.number, number);
	const family = enumAt(body.family, "$.preset.body.family", [
		...PRESET_FAMILIES,
		"All",
	]);
	if (normalizePresetFamily(family) !== request.address.family)
		throw mismatch("$.preset.body.family", request.address.family, family);
	valuesAt(body.values, "$.preset.body.values");
	if ("group_values" in body)
		valuesAt(body.group_values, "$.preset.body.group_values");
	return { ...body, family } as ShowObject<"preset">["body"];
}

function valuesAt(value: unknown, path: string) {
	const values = recordAt(value, path);
	for (const [owner, attributes] of Object.entries(values))
		recordAt(attributes, `${path}.${owner}`);
}

function activeContextAt(value: unknown, request: PresetRecallRequest) {
	const context = stringAt(value, "$.active_context");
	const expected = `preset:${presetStorageKey(request.address)}`;
	if (context !== expected)
		throw mismatch("$.active_context", expected, context);
	return context;
}

function validateRequest(request: PresetRecallRequest) {
	printableAt(request.requestId, "$.requestId", 128);
	printableAt(request.presetId, "$.presetId", 256);
	enumAt(request.address.family, "$.address.family", PRESET_FAMILIES);
	const number = integerAt(request.address.number, "$.address.number");
	if (number < 1 || number > 4_294_967_295)
		throw new WireValidationError(
			"$.address.number",
			"positive 32-bit integer",
			request.address.number,
		);
	for (const [path, revision] of [
		["expectedPresetRevision", request.expectedPresetRevision],
		["expectedShowRevision", request.expectedShowRevision],
		["expectedProgrammerRevision", request.expectedProgrammerRevision],
		["expectedCaptureModeRevision", request.expectedCaptureModeRevision],
		["expectedSelectionRevision", request.expectedSelectionRevision],
		["selectedFixtureCount", request.selectedFixtureCount],
	] as const)
		integerAt(revision, `$.${path}`);
}

function assertOutcomeFields(response: Record<string, unknown>) {
	assertOptionalFields(response, [
		"request_id",
		"correlation_id",
		"replayed",
		"show_revision",
		"programmer_revision",
		"capture_mode_revision",
		"selection_revision",
		"interaction_event_sequence",
		"applied_fixtures",
		"active_context",
		"preset",
		"status",
		"projection",
		"event_sequence",
		"warning",
	]);
}

function assertOptionalFields(
	value: Record<string, unknown>,
	allowed: readonly string[],
) {
	exactRecordAt(
		value,
		"$",
		allowed.filter((key) => key in value),
	);
}

function optionalInteger(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : integerAt(object[key], `${path}.${key}`);
}

function optionalString(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : stringAt(object[key], `${path}.${key}`);
}

function exactRevision(value: unknown, path: string, expected: number) {
	const revision = integerAt(value, path);
	if (revision !== expected) throw mismatch(path, expected, revision);
	return revision;
}

function printableAt(value: unknown, path: string, byteLimit: number) {
	const text = stringAt(value, path);
	if (
		!text.trim() ||
		new TextEncoder().encode(text).length > byteLimit ||
		/\p{Cc}/u.test(text)
	)
		throw new WireValidationError(
			path,
			`1-${byteLimit} printable bytes`,
			value,
		);
	return text;
}

function uuidAt(value: unknown, path: string) {
	const uuid = stringAt(value, path);
	if (
		!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
			uuid,
		)
	)
		throw new WireValidationError(path, "hyphenated UUID", value);
	return uuid;
}

function mismatch(path: string, expected: unknown, actual: unknown) {
	return new WireValidationError(path, String(expected), actual);
}
