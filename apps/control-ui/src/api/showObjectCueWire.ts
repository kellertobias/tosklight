import type { Cue, CueList } from "./types";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeAttributeValue } from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export function decodeCueListBody(
	value: unknown,
	path: string,
	objectId?: string,
): CueList {
	const body = recordAt(value, path);
	const id = stringAt(body.id, `${path}.id`);
	if (objectId != null && id !== objectId)
		invalid(`${path}.id`, `object ID ${objectId}`, id);
	return {
		...body,
		id,
		name: plainStringAt(body.name, `${path}.name`),
		priority: signedIntegerAt(body.priority, `${path}.priority`),
		mode: enumAt(body.mode, `${path}.mode`, ["sequence", "chaser"]),
		looped: booleanAt(body.looped, `${path}.looped`),
		intensity_priority_mode: optionalEnum(
			body,
			"intensity_priority_mode",
			path,
			["htp", "ltp"],
			"htp",
		),
		wrap_mode: optionalNullableEnum(body, "wrap_mode", path, [
			"off",
			"tracking",
			"reset",
		]),
		restart_mode: optionalEnum(
			body,
			"restart_mode",
			path,
			["first_cue", "continue_current_cue"],
			"first_cue",
		),
		force_cue_timing: optionalBoolean(body, "force_cue_timing", path, false),
		disable_cue_timing: optionalBoolean(
			body,
			"disable_cue_timing",
			path,
			false,
		),
		chaser_step_millis: optionalInteger(
			body,
			"chaser_step_millis",
			path,
			1000,
		),
		chaser_xfade_millis: optionalInteger(
			body,
			"chaser_xfade_millis",
			path,
			0,
		),
		chaser_xfade_percent: optionalIntegerOrUndefined(
			body,
			"chaser_xfade_percent",
			path,
		),
		speed_group: optionalNullableString(body, "speed_group", path),
		speed_multiplier: optionalNumber(body, "speed_multiplier", path, 1),
		cues: arrayAt(body.cues, `${path}.cues`).map((cue, index) =>
			decodeCue(cue, `${path}.cues[${index}]`),
		),
	} as CueList;
}

function decodeCue(value: unknown, path: string): Cue {
	const cue = recordAt(value, path);
	return {
		...cue,
		id: stringAt(cue.id, `${path}.id`),
		number: positiveNumberAt(cue.number, `${path}.number`),
		name: plainStringAt(cue.name, `${path}.name`),
		fade_millis: integerAt(cue.fade_millis, `${path}.fade_millis`),
		delay_millis: integerAt(cue.delay_millis, `${path}.delay_millis`),
		trigger: decodeTrigger(cue.trigger, `${path}.trigger`),
		cue_only: optionalBoolean(cue, "cue_only", path, false),
		changes: arrayAt(cue.changes, `${path}.changes`).map((change, index) =>
			decodeCueChange(change, `${path}.changes[${index}]`, "fixture_id"),
		),
		group_changes: arrayAt(cue.group_changes ?? [], `${path}.group_changes`).map(
			(change, index) =>
				decodeCueChange(change, `${path}.group_changes[${index}]`, "group_id"),
		),
		phasers: arrayAt(cue.phasers ?? [], `${path}.phasers`),
	} as Cue;
}

function decodeCueChange(
	value: unknown,
	path: string,
	idKey: "fixture_id" | "group_id",
) {
	const change = recordAt(value, path);
	return {
		...change,
		[idKey]: stringAt(change[idKey], `${path}.${idKey}`),
		attribute: stringAt(change.attribute, `${path}.attribute`),
		value:
			change.value == null
				? null
				: decodeAttributeValue(change.value, `${path}.value`),
		automatic_restore: optionalBoolean(
			change,
			"automatic_restore",
			path,
			false,
		),
		...decodeOptionalMillis(change, "fade_millis", path),
		...decodeOptionalMillis(change, "delay_millis", path),
	};
}

function decodeTrigger(value: unknown, path: string) {
	const trigger = recordAt(value, path);
	const type = enumAt(trigger.type, `${path}.type`, [
		"manual",
		"follow",
		"wait",
		"timecode",
	]);
	if (type === "manual") return { ...trigger, type };
	if (type === "timecode")
		return { ...trigger, type, frame: integerAt(trigger.frame, `${path}.frame`) };
	return {
		...trigger,
		type,
		delay_millis: integerAt(trigger.delay_millis, `${path}.delay_millis`),
	};
}

function decodeOptionalMillis(
	value: Record<string, unknown>,
	key: string,
	path: string,
) {
	return value[key] == null
		? {}
		: { [key]: integerAt(value[key], `${path}.${key}`) };
}

function positiveNumberAt(value: unknown, path: string) {
	const number = numberAt(value, path);
	if (number <= 0) invalid(path, "positive number", value);
	return number;
}

function signedIntegerAt(value: unknown, path: string) {
	const number = numberAt(value, path);
	if (!Number.isSafeInteger(number)) invalid(path, "safe integer", value);
	return number;
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string") invalid(path, "string", value);
	return value;
}

function optionalNullableString(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : stringAt(object[key], `${path}.${key}`);
}

function optionalBoolean(
	object: Record<string, unknown>,
	key: string,
	path: string,
	fallback: boolean,
) {
	return object[key] == null ? fallback : booleanAt(object[key], `${path}.${key}`);
}

function optionalInteger(
	object: Record<string, unknown>,
	key: string,
	path: string,
	fallback: number,
) {
	return object[key] == null ? fallback : integerAt(object[key], `${path}.${key}`);
}

function optionalIntegerOrUndefined(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? undefined : integerAt(object[key], `${path}.${key}`);
}

function optionalNumber(
	object: Record<string, unknown>,
	key: string,
	path: string,
	fallback: number,
) {
	return object[key] == null ? fallback : numberAt(object[key], `${path}.${key}`);
}

function optionalEnum<const T extends string>(
	object: Record<string, unknown>,
	key: string,
	path: string,
	values: readonly T[],
	fallback: T,
) {
	return object[key] == null
		? fallback
		: enumAt(object[key], `${path}.${key}`, values);
}

function optionalNullableEnum<const T extends string>(
	object: Record<string, unknown>,
	key: string,
	path: string,
	values: readonly T[],
) {
	return object[key] == null ? undefined : enumAt(object[key], `${path}.${key}`, values);
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
