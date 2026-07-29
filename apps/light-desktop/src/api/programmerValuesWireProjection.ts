import type {
	ProgrammerDynamicValue,
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
	ProgrammerValuesProjection,
} from "../features/programmerValues/contracts";
import type {
	DynamicDefinitionProjection,
	DynamicInstanceOverridesProjection,
	DynamicReferenceProjection,
	DynamicValueTimingProjection,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import type { AttributeValue } from "./types/playback";
import { WireValidationError } from "./wireValidation";

const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function decodeProgrammerValuesProjection(
	value: unknown,
	path: string,
	expectedUserId: string,
): ProgrammerValuesProjection {
	programmerValuesUuidAt(expectedUserId, "$.requested_user_id");
	const projection = exactRecordAt(value, path, [
		"user_id",
		"revision",
		"fixture_values",
		"group_values",
		"dynamic_definitions",
		"dynamic_values",
	]);
	const userId = programmerValuesUuidAt(projection.user_id, `${path}.user_id`);
	assertExpectedUser(userId, expectedUserId, `${path}.user_id`);
	const fixtureValues = arrayAt(
		projection.fixture_values,
		`${path}.fixture_values`,
	).map((item, index) =>
		decodeFixtureValue(item, `${path}.fixture_values[${index}]`),
	);
	const groupValues = arrayAt(
		projection.group_values,
		`${path}.group_values`,
	).map((item, index) =>
		decodeGroupValue(item, `${path}.group_values[${index}]`),
	);
	const dynamicDefinitions = (
		projection.dynamic_definitions == null
			? []
			: arrayAt(
					projection.dynamic_definitions,
					`${path}.dynamic_definitions`,
				)
	).map((definition, index) =>
		decodeEmbeddedDefinition(
			definition,
			`${path}.dynamic_definitions[${index}]`,
		),
	);
	const definitionsByFallback = new Map(
		dynamicDefinitions.map((definition) => [
			fallbackKey(definition.id, definition.revision),
			definition,
		]),
	);
	const dynamicValues = arrayAt(
		projection.dynamic_values,
		`${path}.dynamic_values`,
	).map((item, index) =>
		decodeDynamicValue(
			item,
			`${path}.dynamic_values[${index}]`,
			definitionsByFallback,
		),
	);
	assertUniqueAddresses(fixtureValues, groupValues, path);
	return {
		userId,
		revision: integerAt(projection.revision, `${path}.revision`),
		fixtureValues,
		groupValues,
		...(dynamicValues.length > 0 ? { dynamicValues } : {}),
	};
}

function decodeDynamicValue(
	value: unknown,
	path: string,
	definitionsByFallback: ReadonlyMap<string, DynamicDefinitionProjection>,
): ProgrammerDynamicValue {
	const item = exactRecordAt(value, path, [
		"fixture_id",
		"attribute",
		"value",
		"programmer_order",
		"changed_at_millis",
	]);
	return {
		fixtureId: programmerValuesUuidAt(item.fixture_id, `${path}.fixture_id`),
		attribute: stringAt(item.attribute, `${path}.attribute`),
		value: decodeDynamicSemanticValue(
			item.value,
			`${path}.value`,
			definitionsByFallback,
		),
		programmerOrder: integerAt(
			item.programmer_order,
			`${path}.programmer_order`,
		),
		changedAtMillis: integerAt(
			item.changed_at_millis,
			`${path}.changed_at_millis`,
		),
	};
}

function decodeDynamicSemanticValue(
	value: unknown,
	path: string,
	definitionsByFallback: ReadonlyMap<string, DynamicDefinitionProjection>,
): ProgrammerDynamicValue["value"] {
	const tagged = recordAt(value, path);
	const type = enumAt(tagged.type, `${path}.type`, [
		"static",
		"dynamic_on",
		"dynamic_off",
		"fix_at",
		"release",
	]);
	if (type === "release")
		return exactRecordAt(value, path, ["type"]) as { type: "release" };
	if (type === "static") {
		const semantic = exactRecordAt(value, path, ["type", "value", "timing"]);
		return {
			type,
			value: decodeAttributeValue(semantic.value, `${path}.value`),
			timing: decodeDynamicTiming(semantic.timing, `${path}.timing`),
		};
	}
	if (type === "fix_at") {
		const semantic = exactRecordAt(value, path, ["type", "value", "timing"]);
		return {
			type,
			value: numberAt(semantic.value, `${path}.value`),
			timing: decodeDynamicTiming(semantic.timing, `${path}.timing`),
		};
	}
	if (type === "dynamic_off") {
		const semantic = exactRecordAt(value, path, [
			"type",
			"instance_link",
			"timing",
		]);
		return {
			type,
			instance_link: programmerValuesUuidAt(
				semantic.instance_link,
				`${path}.instance_link`,
			),
			timing: decodeDynamicTiming(semantic.timing, `${path}.timing`),
		};
	}
	const semantic = exactRecordAt(value, path, [
		"type",
		"instance_link",
		"dynamic",
		"lane_id",
		"overrides",
		"timing",
	]);
	return {
		type,
		instance_link: programmerValuesUuidAt(
			semantic.instance_link,
			`${path}.instance_link`,
		),
		dynamic: decodeDynamicReference(
			semantic.dynamic,
			`${path}.dynamic`,
			definitionsByFallback,
		),
		lane_id: programmerValuesUuidAt(semantic.lane_id, `${path}.lane_id`),
		overrides: decodeDynamicOverrides(semantic.overrides, `${path}.overrides`),
		timing: decodeDynamicTiming(semantic.timing, `${path}.timing`),
	};
}

function decodeDynamicTiming(
	value: unknown,
	path: string,
): DynamicValueTimingProjection {
	const timing = exactRecordAt(value, path, ["fade_millis", "delay_millis"]);
	return {
		fade_millis: optionalMillis(timing, "fade_millis", path),
		delay_millis: optionalMillis(timing, "delay_millis", path),
	};
}

function decodeDynamicReference(
	value: unknown,
	path: string,
	definitionsByFallback: ReadonlyMap<string, DynamicDefinitionProjection>,
): DynamicReferenceProjection & {
	embedded_fallback: DynamicDefinitionProjection;
} {
	const reference = exactRecordAt(value, path, [
		"dynamic_id",
		"last_known_pool_number",
		"embedded_fallback_id",
		"embedded_fallback_revision",
		"embedded_fallback",
	]);
	const inlineDefinition =
		reference.embedded_fallback == null
			? null
			: decodeEmbeddedDefinition(
					reference.embedded_fallback,
					`${path}.embedded_fallback`,
				);
	const fallbackId =
		reference.embedded_fallback_id == null
			? inlineDefinition?.id
			: programmerValuesUuidAt(
					reference.embedded_fallback_id,
					`${path}.embedded_fallback_id`,
				);
	const fallbackRevision =
		reference.embedded_fallback_revision == null
			? inlineDefinition?.revision
			: integerAt(
					reference.embedded_fallback_revision,
					`${path}.embedded_fallback_revision`,
				);
	if (fallbackId == null || fallbackRevision == null)
		throw new WireValidationError(
			path,
			"Dynamic fallback identity or inline definition",
			value,
		);
	const definition =
		inlineDefinition ??
		definitionsByFallback.get(fallbackKey(fallbackId, fallbackRevision));
	if (!definition)
		throw new WireValidationError(
			`${path}.embedded_fallback`,
			`fallback ${fallbackId} revision ${fallbackRevision}`,
			reference.embedded_fallback,
		);
	return {
		dynamic_id:
			reference.dynamic_id == null
				? null
				: programmerValuesUuidAt(reference.dynamic_id, `${path}.dynamic_id`),
		last_known_pool_number: positiveIntegerAt(
			reference.last_known_pool_number,
			`${path}.last_known_pool_number`,
		),
		embedded_fallback_id: fallbackId,
		embedded_fallback_revision: fallbackRevision,
		embedded_fallback: definition,
	};
}

function decodeEmbeddedDefinition(
	value: unknown,
	path: string,
): DynamicDefinitionProjection {
	const definition = exactRecordAt(value, path, [
		"id",
		"pool_number",
		"revision",
		"name",
		"color",
		"icon",
		"target_binding",
		"lanes",
		"random_groups",
		"phase_mode",
		"phase",
		"speed",
		"overall_speed_multiplier",
		"run_mode",
		"default_activation",
		"activation_boundary",
	]);
	programmerValuesUuidAt(definition.id, `${path}.id`);
	arrayAt(definition.lanes, `${path}.lanes`);
	arrayAt(definition.random_groups, `${path}.random_groups`);
	return definition as unknown as DynamicDefinitionProjection;
}

function fallbackKey(id: string, revision: number) {
	return `${id}:${revision}`;
}

function decodeDynamicOverrides(
	value: unknown,
	path: string,
): DynamicInstanceOverridesProjection {
	const overrides = exactRecordAt(value, path, [
		"size",
		"speed_multiplier",
		"phase_offset_degrees",
	]);
	const multiplier = exactRecordAt(
		overrides.speed_multiplier,
		`${path}.speed_multiplier`,
		["numerator", "denominator"],
	);
	const size = nonNegativeAt(overrides.size, `${path}.size`);
	const numerator = positiveIntegerAt(
		multiplier.numerator,
		`${path}.speed_multiplier.numerator`,
	);
	const denominator = positiveIntegerAt(
		multiplier.denominator,
		`${path}.speed_multiplier.denominator`,
	);
	return {
		size,
		speed_multiplier: { numerator, denominator },
		phase_offset_degrees: numberAt(
			overrides.phase_offset_degrees,
			`${path}.phase_offset_degrees`,
		),
	};
}

function positiveIntegerAt(value: unknown, path: string) {
	const decoded = integerAt(value, path);
	if (decoded <= 0)
		throw new WireValidationError(path, "positive integer", value);
	return decoded;
}

function decodeFixtureValue(
	value: unknown,
	path: string,
): ProgrammerFixtureValue {
	const item = exactRecordAt(value, path, [
		"fixture_id",
		"attribute",
		"value",
		"programmer_order",
		"fade",
		"fade_millis",
		"delay_millis",
	]);
	return {
		fixtureId: programmerValuesUuidAt(item.fixture_id, `${path}.fixture_id`),
		attribute: stringAt(item.attribute, `${path}.attribute`),
		value: decodeAttributeValue(item.value, `${path}.value`),
		programmerOrder: integerAt(
			item.programmer_order,
			`${path}.programmer_order`,
		),
		...decodeTiming(item, path),
	};
}

function decodeGroupValue(value: unknown, path: string): ProgrammerGroupValue {
	const item = exactRecordAt(value, path, [
		"group_id",
		"attribute",
		"value",
		"programmer_order",
		"fade",
		"fade_millis",
		"delay_millis",
	]);
	return {
		groupId: stringAt(item.group_id, `${path}.group_id`),
		attribute: stringAt(item.attribute, `${path}.attribute`),
		value: decodeAttributeValue(item.value, `${path}.value`),
		programmerOrder: integerAt(
			item.programmer_order,
			`${path}.programmer_order`,
		),
		...decodeTiming(item, path),
	};
}

function decodeTiming(item: Record<string, unknown>, path: string) {
	return {
		fade: booleanAt(item.fade, `${path}.fade`),
		fadeMillis: optionalMillis(item, "fade_millis", path),
		delayMillis: optionalMillis(item, "delay_millis", path),
	};
}

function optionalMillis(
	item: Record<string, unknown>,
	key: string,
	path: string,
): number | null {
	const value = item[key];
	return value == null ? null : integerAt(value, `${path}.${key}`);
}

export function decodeAttributeValue(
	value: unknown,
	path: string,
): AttributeValue {
	const attribute = exactRecordAt(value, path, ["kind", "value"]);
	const kind = enumAt(attribute.kind, `${path}.kind`, [
		"normalized",
		"spread",
		"discrete",
		"color_xyz",
		"raw_dmx",
		"raw_dmx_exact",
	]);
	if (kind === "normalized")
		return { kind, value: normalizedAt(attribute.value, `${path}.value`) };
	if (kind === "spread")
		return {
			kind,
			value: decodeSpread(attribute.value, `${path}.value`),
		};
	if (kind === "discrete")
		return { kind, value: stringAt(attribute.value, `${path}.value`) };
	if (kind === "color_xyz")
		return { kind, value: decodeColor(attribute.value, `${path}.value`) };
	const raw = integerAt(attribute.value, `${path}.value`);
	const maximum = kind === "raw_dmx" ? 255 : 4_294_967_295;
	if (raw > maximum)
		throw new WireValidationError(
			`${path}.value`,
			`integer <= ${maximum}`,
			raw,
		);
	return { kind, value: raw };
}

function normalizedAt(value: unknown, path: string) {
	const level = numberAt(value, path);
	if (level < 0 || level > 1)
		throw new WireValidationError(path, "number between 0 and 1", value);
	return level;
}

function decodeSpread(value: unknown, path: string) {
	const values = arrayAt(value, path).map((item, index) =>
		normalizedAt(item, `${path}[${index}]`),
	);
	if (values.length < 2)
		throw new WireValidationError(
			path,
			"at least two normalized values",
			value,
		);
	return values;
}

function decodeColor(value: unknown, path: string) {
	const color = exactRecordAt(value, path, ["x", "y", "z"]);
	return {
		x: nonNegativeAt(color.x, `${path}.x`),
		y: nonNegativeAt(color.y, `${path}.y`),
		z: nonNegativeAt(color.z, `${path}.z`),
	};
}

function nonNegativeAt(value: unknown, path: string) {
	const number = numberAt(value, path);
	if (number < 0)
		throw new WireValidationError(path, "non-negative number", value);
	return number;
}

function assertUniqueAddresses(
	fixtureValues: readonly ProgrammerFixtureValue[],
	groupValues: readonly ProgrammerGroupValue[],
	path: string,
) {
	const addresses = new Set<string>();
	for (const value of fixtureValues)
		addAddress(
			addresses,
			`fixture:${value.fixtureId}:${value.attribute}`,
			path,
		);
	for (const value of groupValues)
		addAddress(addresses, `group:${value.groupId}:${value.attribute}`, path);
}

function addAddress(addresses: Set<string>, address: string, path: string) {
	if (addresses.has(address))
		throw new WireValidationError(
			path,
			"unique Programmer value addresses",
			address,
		);
	addresses.add(address);
}

export function programmerValuesUuidAt(value: unknown, path: string): string {
	const decoded = stringAt(value, path);
	if (!UUID_PATTERN.test(decoded))
		throw new WireValidationError(path, "hyphenated UUID", value);
	return decoded;
}

function assertExpectedUser(actual: string, expected: string, path: string) {
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw new WireValidationError(path, `requested user ${expected}`, actual);
}
