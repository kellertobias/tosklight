import type {
	PlaybackButtonAction,
	PlaybackDefinition,
	PlaybackPage,
} from "./types";
import type { PlaybackTopologyDynamicAssignment } from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	MAX_VIRTUAL_PLAYBACK_NUMBER,
	isVirtualPlaybackNumberForPage,
	virtualPlaybackBankStart,
} from "./virtualPlaybackAddress";
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
	"dynamic_restart",
	"dynamic_double_speed",
	"dynamic_half_speed",
	"dynamic_learn_speed",
	"none",
] as const satisfies readonly PlaybackButtonAction[];

export function decodePlaybackBody(
	value: unknown,
	path: string,
	_objectId?: string,
	maximumNumber = 1_000,
): PlaybackDefinition {
	const playback = recordAt(value, path);
	// Legacy portable objects may retain a non-numeric storage ID; `number` is the desk identity.
	const number = positiveIntegerAt(
		playback.number,
		`${path}.number`,
		maximumNumber,
	);
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
	const footprint = decodeFootprint(playback.footprint, `${path}.footprint`);
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
		footprint,
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

function decodeFootprint(
	value: unknown,
	path: string,
): NonNullable<PlaybackDefinition["footprint"]> {
	if (value == null) return { type: "normal" };
	const footprint = recordAt(value, path);
	const type = enumAt(footprint.type, `${path}.type`, [
		"normal",
		"taller",
		"wider",
	]);
	if (type === "normal") return { type };
	if (type === "taller")
		return {
			type,
			upper_button: enumAt(
				footprint.upper_button,
				`${path}.upper_button`,
				BUTTON_ACTIONS,
			),
		};
	const buttons = arrayAt(footprint.right_buttons, `${path}.right_buttons`);
	if (buttons.length !== 3)
		invalid(`${path}.right_buttons`, "three actions", buttons);
	return {
		type,
		right_buttons: buttons.map((button, index) =>
			enumAt(button, `${path}.right_buttons[${index}]`, BUTTON_ACTIONS),
		) as PlaybackDefinition["buttons"],
		right_fader: enumAt(footprint.right_fader, `${path}.right_fader`, [
			"master",
			"temp",
			"speed",
			"x_fade",
			"direct_bpm",
			"centered_relative",
			"learned_percentage",
		]),
	};
}

export function decodePlaybackPageBody(
	value: unknown,
	path: string,
	_objectId?: string,
): PlaybackPage {
	const page = recordAt(value, path);
	// Page object keys are lossless storage identities and need not equal the page number.
	const pageNumber = positiveIntegerAt(page.number, `${path}.number`, 127);
	const slots = recordAt(page.slots ?? {}, `${path}.slots`);
	const decodedSlots = Object.fromEntries(
		Object.entries(slots).map(([slot, number]) => [
			String(positiveIntegerAt(Number(slot), `${path}.slots.${slot}`, 127)),
			positiveIntegerAt(number, `${path}.slots.${slot}`, 1000),
		]),
	);
	const virtualPlaybacks = recordAt(
		page.virtual_playbacks,
		`${path}.virtual_playbacks`,
	);
	const decodedVirtualPlaybacks = Object.fromEntries(
		Object.entries(virtualPlaybacks).map(([number, playback]) => {
			const decodedNumber = positiveIntegerAt(
				Number(number),
				`${path}.virtual_playbacks.${number}`,
				MAX_VIRTUAL_PLAYBACK_NUMBER,
			);
			if (!isVirtualPlaybackNumberForPage(pageNumber, decodedNumber))
				throw new WireValidationError(
					`${path}.virtual_playbacks.${number}`,
					`integer in page ${pageNumber}'s bank ${virtualPlaybackBankStart(pageNumber)}-${virtualPlaybackBankStart(pageNumber) + 299}`,
					number,
				);
			const decoded = decodePlaybackBody(
				playback,
				`${path}.virtual_playbacks.${number}`,
				undefined,
				MAX_VIRTUAL_PLAYBACK_NUMBER,
			);
			if (decoded.number !== decodedNumber)
				throw new WireValidationError(
					`${path}.virtual_playbacks.${number}.number`,
					String(decodedNumber),
					decoded.number,
				);
			return [String(decodedNumber), decoded];
		}),
	);
	return {
		...page,
		number: pageNumber,
		name: plainStringAt(page.name, `${path}.name`),
		slots: decodedSlots,
		virtual_playbacks: decodedVirtualPlaybacks,
	} as PlaybackPage;
}

function decodeTarget(value: unknown, path: string) {
	const target = recordAt(value, path);
	const type = enumAt(target.type, `${path}.type`, [
		"cue_list",
		"group",
		"speed_group",
		"dynamic",
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
			initial_master:
				target.initial_master == null
					? target.initial_master
					: boundedMasterAt(
							target.initial_master,
							`${path}.initial_master`,
						),
		};
	if (type === "speed_group")
		return { ...target, type, group: stringAt(target.group, `${path}.group`) };
	if (type === "dynamic")
		return {
			type,
			assignment: decodeDynamicAssignment(
				target.assignment,
				`${path}.assignment`,
			),
		};
	return { ...target, type };
}

function decodeDynamicAssignment(
	value: unknown,
	path: string,
): PlaybackTopologyDynamicAssignment {
	const assignment = recordAt(value, path);
	const dynamic = recordAt(assignment.dynamic, `${path}.dynamic`);
	const fallback = recordAt(
		dynamic.embedded_fallback,
		`${path}.dynamic.embedded_fallback`,
	);
	const definition = recordAt(
		fallback.definition,
		`${path}.dynamic.embedded_fallback.definition`,
	);
	const targetScope =
		assignment.target_scope == null
			? null
			: recordAt(assignment.target_scope, `${path}.target_scope`);
	if (targetScope) {
		const scopeType = enumAt(targetScope.type, `${path}.target_scope.type`, [
			"live_group",
			"frozen_targets",
		]);
		if (scopeType === "live_group")
			stringAt(targetScope.group_id, `${path}.target_scope.group_id`);
		else
			arrayAt(targetScope.targets, `${path}.target_scope.targets`).forEach(
				(target, index) =>
					stringAt(target, `${path}.target_scope.targets[${index}]`),
			);
	}
	const multiplier = recordAt(
		assignment.local_speed_multiplier,
		`${path}.local_speed_multiplier`,
	);
	integerAt(multiplier.numerator, `${path}.local_speed_multiplier.numerator`);
	integerAt(
		multiplier.denominator,
		`${path}.local_speed_multiplier.denominator`,
	);
	return {
		dynamic_id:
			dynamic.dynamic_id == null
				? null
				: stringAt(dynamic.dynamic_id, `${path}.dynamic.dynamic_id`),
		last_known_pool_number: positiveIntegerAt(
			dynamic.last_known_pool_number,
			`${path}.dynamic.last_known_pool_number`,
			9_999,
		),
		embedded_fallback: {
			...definition,
			spatial_mapping:
				definition.spatial_mapping &&
				typeof definition.spatial_mapping === "object"
					? definition.spatial_mapping
					: {
							projection: { type: "inherit" },
							shape: { type: "inherit" },
						},
		},
		revision: positiveIntegerAt(assignment.revision, `${path}.revision`, Number.MAX_SAFE_INTEGER),
		target_scope: targetScope,
		fader_mode: enumAt(assignment.fader_mode, `${path}.fader_mode`, [
			"none",
			"master",
			"size",
			"size_and_master",
		]),
		priority: integerAt(assignment.priority, `${path}.priority`),
		activation_override:
			assignment.activation_override == null
				? null
				: enumAt(
						assignment.activation_override,
						`${path}.activation_override`,
						["start_now", "join_sync_now", "next_boundary"],
					),
		resume_policy: enumAt(assignment.resume_policy, `${path}.resume_policy`, [
			"follow_dynamic",
			"resume_frozen_phase",
			"rejoin_synchronized_position",
			"resume_on_next_boundary",
		]),
		local_speed_multiplier: {
			numerator: multiplier.numerator as number,
			denominator: multiplier.denominator as number,
		},
		learned_duration_millis:
			assignment.learned_duration_millis == null
				? null
				: integerAt(
						assignment.learned_duration_millis,
						`${path}.learned_duration_millis`,
					),
		crossfade_non_intensity: booleanAt(
			assignment.crossfade_non_intensity,
			`${path}.crossfade_non_intensity`,
		),
		auto_off_at_zero: booleanAt(
			assignment.auto_off_at_zero,
			`${path}.auto_off_at_zero`,
		),
		auto_off_flash_release: booleanAt(
			assignment.auto_off_flash_release,
			`${path}.auto_off_flash_release`,
		),
		auto_off_full_control: booleanAt(
			assignment.auto_off_full_control,
			`${path}.auto_off_full_control`,
		),
	} as PlaybackTopologyDynamicAssignment;
}

function defaultButtons(
	target: PlaybackDefinition["target"],
): PlaybackDefinition["buttons"] {
	if (target.type === "cue_list") return ["go_minus", "go", "flash"];
	if (target.type === "group")
		return ["select", "select_dereferenced", "flash"];
	if (target.type === "speed_group") return ["double", "half", "learn"];
	if (target.type === "dynamic")
		return ["off", "toggle", "dynamic_restart"];
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

function boundedMasterAt(value: unknown, path: string) {
	const number = numberAt(value, path);
	if (number < 0 || number > 1) invalid(path, "number between 0 and 1", value);
	return number;
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
