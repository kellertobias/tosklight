import type {
	FixtureChannel,
	FixtureMode,
	FixtureProfile,
} from "../../../api/types";
import { derivePrimarySlots } from "./channels";
import { maxRaw } from "./rawValues";

function validatePhysical(profile: FixtureProfile, errors: string[]) {
	for (const [key, value] of Object.entries(profile.physical)) {
		const permitsZero = key === "color_rendering_index";
		if (
			typeof value === "number" &&
			(!Number.isFinite(value) || (permitsZero ? value < 0 : value <= 0))
		) {
			errors.push(`${key.replaceAll("_", " ")} must be positive`);
		}
	}
	if (
		profile.physical.color_rendering_index != null &&
		profile.physical.color_rendering_index > 100
	) {
		errors.push("color rendering index must be from 0 to 100");
	}
}

function validateSplits(
	profile: FixtureProfile,
	mode: FixtureMode,
	errors: string[],
) {
	const splitNumbers = new Set<number>();
	for (const split of mode.splits) {
		if (
			!Number.isInteger(split.number) ||
			split.number < 1 ||
			splitNumbers.has(split.number)
		) {
			errors.push(
				`${mode.name}: split numbers must be unique positive integers`,
			);
		}
		const validFootprint =
			profile.patch_policy === "visual_only"
				? split.footprint === 0
				: Number.isInteger(split.footprint) &&
					split.footprint >= 1 &&
					split.footprint <= 512;
		if (!validFootprint) {
			errors.push(
				`${mode.name}: split ${split.number} footprint must be ${profile.patch_policy === "visual_only" ? "zero" : "1–512"}`,
			);
		}
		splitNumbers.add(split.number);
	}
	if (!mode.splits.length) {
		errors.push(`${mode.name}: at least one split is required`);
	}
	return splitNumbers;
}

function validateFunctionRanges(
	mode: FixtureMode,
	channel: FixtureChannel,
	maximum: number,
	errors: string[],
) {
	const sorted = [...channel.functions].sort(
		(left, right) => left.dmx_from - right.dmx_from,
	);
	sorted.forEach((fn, index) => {
		if (fn.dmx_from < 0 || fn.dmx_to > maximum || fn.dmx_from > fn.dmx_to) {
			errors.push(
				`${mode.name}: ${fn.name || "function"} has an invalid DMX range`,
			);
		}
		if (index && sorted[index - 1].dmx_to >= fn.dmx_from) {
			errors.push(
				`${mode.name}: ${channel.attribute} function ranges overlap`,
			);
		}
	});
}

function validateChannels(
	mode: FixtureMode,
	headIds: Set<string>,
	splitNumbers: Set<number>,
	errors: string[],
) {
	const channelIds = new Set<string>();
	for (const channel of mode.channels) {
		if (!headIds.has(channel.head_id)) {
			errors.push(
				`${mode.name}: ${channel.attribute || "channel"} references a missing head`,
			);
		}
		if (!splitNumbers.has(channel.split)) {
			errors.push(
				`${mode.name}: ${channel.attribute || "channel"} references missing split ${channel.split}`,
			);
		}
		if (channelIds.has(channel.id)) {
			errors.push(`${mode.name}: channel identities must be unique`);
		}
		channelIds.add(channel.id);
		const maximum = maxRaw(channel.resolution);
		if (!channel.attribute.trim()) {
			errors.push(`${mode.name}: every channel needs an attribute`);
		}
		if (
			!Number.isInteger(channel.default_raw) ||
			channel.default_raw < 0 ||
			channel.default_raw > maximum
		) {
			errors.push(
				`${mode.name}: ${channel.attribute} default must be 0–${maximum}`,
			);
		}
		if (
			!Number.isInteger(channel.highlight_raw) ||
			channel.highlight_raw < 0 ||
			channel.highlight_raw > maximum
		) {
			errors.push(
				`${mode.name}: ${channel.attribute} highlight must be 0–${maximum}`,
			);
		}
		validateFunctionRanges(mode, channel, maximum, errors);
	}
}

function validateMode(
	profile: FixtureProfile,
	mode: FixtureMode,
	errors: string[],
) {
	const splitNumbers = validateSplits(profile, mode, errors);
	if (!mode.heads.length) {
		errors.push(`${mode.name}: at least one head is required`);
	}
	if (mode.heads.filter((head) => head.master_shared).length > 1) {
		errors.push(`${mode.name}: only one head can be master/shared`);
	}
	const headIds = new Set(mode.heads.map((head) => head.id));
	for (const head of mode.heads) {
		if (!head.name.trim()) {
			errors.push(`${mode.name}: every head needs a name`);
		}
	}
	validateChannels(mode, headIds, splitNumbers, errors);
	if (
		profile.patch_policy === "visual_only" &&
		(mode.channels.length ||
			mode.color_systems.length ||
			mode.control_actions.length)
	) {
		errors.push(`${mode.name}: visual-only modes cannot contain DMX behavior`);
	}
	errors.push(
		...derivePrimarySlots(mode).errors.map(
			(error) => `${mode.name}: ${error}`,
		),
	);
}

export function validateProfile(profile: FixtureProfile) {
	const errors: string[] = [];
	if (!profile.manufacturer.trim()) errors.push("Manufacturer is required");
	if (!profile.name.trim()) errors.push("Fixture name is required");
	if (!profile.modes.length) errors.push("At least one mode is required");
	validatePhysical(profile, errors);
	const modeIds = new Set<string>();
	for (const mode of profile.modes) {
		if (!mode.name.trim()) errors.push("Every mode needs a name");
		if (modeIds.has(mode.id)) errors.push("Mode identities must be unique");
		modeIds.add(mode.id);
		validateMode(profile, mode, errors);
	}
	return [...new Set(errors)];
}
