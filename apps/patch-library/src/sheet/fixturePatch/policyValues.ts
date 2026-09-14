import type { PatchedFixture } from "../../wire";
import { fixturePolicyApplicability } from "./patchModel";

export const MASTERS_VALUES = ["none", "group", "grand", "both"] as const;
export type MastersValue = (typeof MASTERS_VALUES)[number];

export function isMastersValue(value: string): value is MastersValue {
	return (MASTERS_VALUES as readonly string[]).includes(value);
}

/**
 * Which masters reduce the fixture, as one value. A master the fixture cannot react to never counts,
 * and a fixture that reacts to neither has no value at all.
 */
export function mastersValue(fixture: PatchedFixture): MastersValue | null {
	const applicable = fixturePolicyApplicability(fixture.definition);
	if (!applicable.groupMasters && !applicable.grandMaster) return null;
	const group =
		applicable.groupMasters && (fixture.group_masters_enabled ?? true);
	const grand = applicable.grandMaster && (fixture.grand_master_enabled ?? true);
	if (group && grand) return "both";
	if (group) return "group";
	if (grand) return "grand";
	return "none";
}

export const MIB_MAX_SECONDS = 30;

/** Move in Black as the editor holds it: `off`, or the delay in seconds. */
export function mibEditValue(fixture: PatchedFixture) {
	return (fixture.move_in_black_enabled ?? true)
		? String((fixture.move_in_black_delay_millis ?? 0) / 1000)
		: "off";
}

/** Move in Black as the table shows it: `Off`, or the delay such as `2.5s`. */
export function formatMib(fixture: PatchedFixture) {
	const value = mibEditValue(fixture);
	return value === "off" ? "Off" : `${value}s`;
}

/** `off`, or a delay from 0 s to 30 s with decimals; anything else is not a Move in Black value. */
export function parseMib(
	raw: string,
): Pick<
	PatchedFixture,
	"move_in_black_enabled" | "move_in_black_delay_millis"
> | null {
	const text = raw.trim().toLowerCase();
	if (text === "off") return { move_in_black_enabled: false };
	const seconds = Number(text.replace(/\s*s$/u, ""));
	if (!text || !Number.isFinite(seconds)) return null;
	if (seconds < 0 || seconds > MIB_MAX_SECONDS) return null;
	return {
		move_in_black_enabled: true,
		move_in_black_delay_millis: Math.round(seconds * 1000),
	};
}
