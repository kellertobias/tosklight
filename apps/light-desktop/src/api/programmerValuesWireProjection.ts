import type { AttributeValue } from "./types/playback";
import type {
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
	ProgrammerValuesProjection,
} from "../features/programmerValues/contracts";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	stringAt,
} from "./playbackWirePrimitives";
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
	assertUniqueAddresses(fixtureValues, groupValues, path);
	return {
		userId,
		revision: integerAt(projection.revision, `${path}.revision`),
		fixtureValues,
		groupValues,
	};
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

export function decodeAttributeValue(value: unknown, path: string): AttributeValue {
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
