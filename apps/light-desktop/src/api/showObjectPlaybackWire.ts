import type {
	PlaybackButtonAction,
	PlaybackDefinition,
	PlaybackPage,
} from "./types";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

const BUTTON_ACTIONS = [
	"on",
	"off",
	"toggle",
	"go",
	"go_minus",
	"fast_forward",
	"fast_rewind",
	"flash",
	"temp",
	"swap",
	"select",
	"select_contents",
	"select_dereferenced",
	"learn",
	"double",
	"half",
	"pause",
	"blackout",
	"pause_dynamics",
	"none",
] as const satisfies readonly PlaybackButtonAction[];

export function decodePlaybackBody(
	value: unknown,
	path: string,
	_objectId?: string,
): PlaybackDefinition {
	const playback = recordAt(value, path);
	// Legacy portable objects may retain a non-numeric storage ID; `number` is the desk identity.
	const number = positiveIntegerAt(playback.number, `${path}.number`, 1000);
	const target = decodeTarget(playback.target, `${path}.target`);
	const buttons = arrayAt(
		playback.buttons ?? defaultButtons(target),
		`${path}.buttons`,
	);
	if (buttons.length !== 3) invalid(`${path}.buttons`, "three actions", buttons);
	const fader = optionalEnum(
		playback,
		"fader",
		path,
		[
			"master",
			"temp",
			"speed",
			"x_fade",
			"direct_bpm",
			"centered_relative",
			"learned_percentage",
		],
		defaultFader(target),
	);
	return {
		...playback,
		number,
		name: plainStringAt(playback.name, `${path}.name`),
		target,
		buttons: buttons.map((button, index) =>
			enumAt(button, `${path}.buttons[${index}]`, BUTTON_ACTIONS),
		) as PlaybackDefinition["buttons"],
		button_count: optionalBoundedInteger(playback, "button_count", path, 3, 3),
		fader:
			target.type === "speed_group" && fader === "speed"
				? "learned_percentage"
				: fader,
		has_fader: optionalBoolean(playback, "has_fader", path, true),
		go_activates: optionalBoolean(playback, "go_activates", path, true),
		auto_off: optionalBoolean(playback, "auto_off", path, true),
		xfade_millis: optionalInteger(playback, "xfade_millis", path, 0),
		color: optionalPlainString(playback, "color", path, "#20c997"),
		flash_release: optionalEnum(
			playback,
			"flash_release",
			path,
			["release_all", "release_intensity_only"],
			"release_all",
		),
		protect_from_swap: optionalBoolean(
			playback,
			"protect_from_swap",
			path,
			false,
		),
		presentation_icon: optionalNullableString(
			playback,
			"presentation_icon",
			path,
		),
		presentation_image: optionalNullableString(
			playback,
			"presentation_image",
			path,
		),
	} as PlaybackDefinition;
}

export function decodePlaybackPageBody(
	value: unknown,
	path: string,
	_objectId?: string,
): PlaybackPage {
	const page = recordAt(value, path);
	// Page object keys are lossless storage identities and need not equal the page number.
	const number = positiveIntegerAt(page.number, `${path}.number`, 127);
	const slots = recordAt(page.slots ?? {}, `${path}.slots`);
	const decodedSlots = Object.fromEntries(
		Object.entries(slots).map(([slot, number]) => [
			String(positiveIntegerAt(Number(slot), `${path}.slots.${slot}`, 127)),
			positiveIntegerAt(number, `${path}.slots.${slot}`, 1000),
		]),
	);
	return {
		...page,
		number,
		name: plainStringAt(page.name, `${path}.name`),
		slots: decodedSlots,
	} as PlaybackPage;
}

function decodeTarget(value: unknown, path: string) {
	const target = recordAt(value, path);
	const type = enumAt(target.type, `${path}.type`, [
		"cue_list",
		"group",
		"speed_group",
		"programmer_fade",
		"cue_fade",
		"grand_master",
	]);
	if (type === "cue_list")
		return {
			...target,
			type,
			cue_list_id: stringAt(target.cue_list_id, `${path}.cue_list_id`),
		};
	if (type === "group")
		return {
			...target,
			type,
			group_id: stringAt(target.group_id, `${path}.group_id`),
		};
	if (type === "speed_group")
		return { ...target, type, group: stringAt(target.group, `${path}.group`) };
	return { ...target, type };
}

function defaultButtons(
	target: PlaybackDefinition["target"],
): PlaybackDefinition["buttons"] {
	if (target.type === "cue_list") return ["go_minus", "go", "flash"];
	if (target.type === "group")
		return ["select", "select_dereferenced", "flash"];
	if (target.type === "speed_group") return ["double", "half", "learn"];
	if (target.type === "programmer_fade" || target.type === "cue_fade")
		return ["double", "half", "off"];
	return ["blackout", "pause_dynamics", "flash"];
}

function defaultFader(target: PlaybackDefinition["target"]) {
	return target.type === "speed_group" ? "learned_percentage" : "master";
}

function positiveIntegerAt(value: unknown, path: string, maximum: number) {
	const integer = integerAt(value, path);
	if (integer < 1 || integer > maximum)
		invalid(path, `integer between 1 and ${maximum}`, value);
	return integer;
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string") invalid(path, "string", value);
	return value;
}

function optionalPlainString(
	object: Record<string, unknown>,
	key: string,
	path: string,
	fallback: string,
) {
	return object[key] == null ? fallback : plainStringAt(object[key], `${path}.${key}`);
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

function optionalBoundedInteger(
	object: Record<string, unknown>,
	key: string,
	path: string,
	fallback: number,
	maximum: number,
) {
	const integer = optionalInteger(object, key, path, fallback);
	if (integer > maximum) invalid(`${path}.${key}`, `integer <= ${maximum}`, integer);
	return integer;
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

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
