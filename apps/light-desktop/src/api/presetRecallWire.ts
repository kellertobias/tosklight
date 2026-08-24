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
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeProgrammerPreloadValuesProjection } from "./programmerPreloadValuesWireProjection";
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
		address: {
			family: WIRE_FAMILY[request.address.family],
			number: request.address.number,
		},
		expected_preset_revision: request.expectedPresetRevision,
		expected_show_revision: request.expectedShowRevision,
		expected_programmer_revision: request.expectedProgrammerRevision,
		...(request.expectedPreloadValuesRevision === null
			? {}
			: {
					expected_preload_values_revision:
						request.expectedPreloadValuesRevision,
				}),
		expected_capture_mode_revision: request.expectedCaptureModeRevision,
		expected_selection_revision: request.expectedSelectionRevision,
	};
}

export function decodePresetRecallOutcome(
	value: unknown,
	expectedRequest: PresetRecallRequest,
): PresetRecallOutcome {
	const response = recordAt(value, "$");
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	const disposition = enumAt(response.disposition, "$.disposition", [
		"recalled",
		"targets_selected",
	]);
	const target =
		response.target == null
			? "programmer"
			: enumAt(response.target, "$.target", ["programmer", "preload"]);
	assertOutcomeFields(response);
	assertIsolatedValuesFields(response, target);
	const projection = optionalProjection(
		response,
		expectedRequest,
		status,
		target,
	);
	const eventSequence = optionalInteger(
		response,
		target === "preload" ? "preload_event_sequence" : "event_sequence",
		"$",
	);
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
		target,
		correlationId: uuidAt(response.correlation_id, "$.correlation_id"),
		disposition,
		showRevision: exactRevision(
			response.show_revision,
			"$.show_revision",
			expectedRequest.expectedShowRevision,
		),
		programmerRevision: programmerRevision(
			response.programmer_revision,
			target === "programmer" ? projection : null,
			expectedRequest,
		),
		preloadValuesRevision: preloadValuesRevision(
			response.preload_values_revision,
			target,
			target === "preload" ? projection : null,
			expectedRequest,
		),
		captureModeRevision: exactRevision(
			response.capture_mode_revision,
			"$.capture_mode_revision",
			expectedRequest.expectedCaptureModeRevision,
		),
		selectionRevision,
		interactionEventSequence,
		appliedFixtures: integerAt(response.applied_fixtures, "$.applied_fixtures"),
		selectedTargets: integerAt(response.selected_targets, "$.selected_targets"),
		activeContext: activeContextAt(
			response.active_context,
			expectedRequest,
			disposition,
			target,
		),
		preset: decodeRecalledPreset(response.preset, expectedRequest),
		warning: optionalString(response, "warning", "$"),
	};
	assertDispositionCounts(base, expectedRequest);
	return status === "changed"
		? { ...base, status, projection, eventSequence }
		: { ...base, status, projection: null, eventSequence: null };
}

function assertIsolatedValuesFields(
	response: Record<string, unknown>,
	target: "programmer" | "preload",
) {
	const inactive =
		target === "preload"
			? ["projection", "event_sequence"]
			: [
					"preload_values_revision",
					"preload_projection",
					"preload_event_sequence",
				];
	for (const key of inactive)
		if (response[key] != null)
			throw new WireValidationError(
				`$.${key}`,
				`absent for ${target} target`,
				response[key],
			);
}

function assertDispositionCounts(
	outcome: {
		disposition: "recalled" | "targets_selected";
		appliedFixtures: number;
		selectedTargets: number;
	},
	request: PresetRecallRequest,
) {
	const expectedApplied =
		outcome.disposition === "recalled" ? request.selectedFixtureCount : 0;
	if (outcome.appliedFixtures !== expectedApplied)
		throw mismatch(
			"$.applied_fixtures",
			expectedApplied,
			outcome.appliedFixtures,
		);
	if (outcome.disposition === "recalled" && outcome.selectedTargets !== 0)
		throw mismatch("$.selected_targets", 0, outcome.selectedTargets);
	if (
		outcome.disposition === "targets_selected" &&
		request.selectedFixtureCount !== 0
	)
		throw mismatch(
			"$.disposition",
			"recalled for a non-empty selection",
			outcome.disposition,
		);
}

function optionalProjection(
	response: Record<string, unknown>,
	request: PresetRecallRequest,
	status: "changed" | "no_change",
	target: "programmer" | "preload",
) {
	const key = target === "preload" ? "preload_projection" : "projection";
	if (response[key] == null) return null;
	if (status === "no_change")
		throw new WireValidationError(
			"$.projection",
			"absent for no_change",
			response[key],
		);
	const projection =
		target === "preload"
			? decodeProgrammerPreloadValuesProjection(response[key], `$.${key}`)
			: decodeProgrammerValuesProjection(response[key], `$.${key}`);
	const expectedRevision =
		target === "preload"
			? (request.expectedPreloadValuesRevision ?? -1) + 1
			: request.expectedProgrammerRevision + 1;
	if (projection.revision !== expectedRevision)
		throw mismatch(`$.${key}.revision`, expectedRevision, projection.revision);
	return projection;
}

function preloadValuesRevision(
	value: unknown,
	target: "programmer" | "preload",
	projection: PresetRecallOutcome["projection"],
	request: PresetRecallRequest,
) {
	if (target === "programmer") {
		if (value != null)
			throw new WireValidationError(
				"$.preload_values_revision",
				"absent for Programmer target",
				value,
			);
		return null;
	}
	if (request.expectedPreloadValuesRevision === null)
		throw new WireValidationError(
			"$.target",
			"programmer when no Preload revision was captured",
			target,
		);
	const revision = integerAt(value, "$.preload_values_revision");
	const expected =
		projection?.revision ?? request.expectedPreloadValuesRevision;
	if (revision !== expected)
		throw mismatch("$.preload_values_revision", expected, revision);
	return revision;
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

function activeContextAt(
	value: unknown,
	request: PresetRecallRequest,
	disposition: "recalled" | "targets_selected",
	target: "programmer" | "preload",
) {
	if (disposition === "targets_selected" || target === "preload")
		return value == null ? null : stringAt(value, "$.active_context");
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
	if (request.expectedPreloadValuesRevision !== null)
		integerAt(
			request.expectedPreloadValuesRevision,
			"$.expectedPreloadValuesRevision",
		);
}

function assertOutcomeFields(response: Record<string, unknown>) {
	assertOptionalFields(response, [
		"correlation_id",
		"disposition",
		"show_revision",
		"programmer_revision",
		"target",
		"preload_values_revision",
		"capture_mode_revision",
		"selection_revision",
		"interaction_event_sequence",
		"applied_fixtures",
		"selected_targets",
		"active_context",
		"preset",
		"status",
		"projection",
		"event_sequence",
		"preload_projection",
		"preload_event_sequence",
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
