import type { PatchedFixture } from "../../wire";
import { sceneryOf } from "./scenerySize";

export type SceneryOptions = NonNullable<PatchedFixture["scenery_options"]>;
export type ChainTopEnd = NonNullable<SceneryOptions["chain_top"]>;
export type ChainBottomEnd = NonNullable<SceneryOptions["chain_bottom"]>;
export type ChainMode = "plain" | "motor_top" | "motor_bottom";
export type SceneryOptionEdit = "scenery_colour" | "chain";

/** The patch sheet columns these choices are shown in, in table order. */
export const SCENERY_OPTION_COLUMNS = [
	"scenery_colour",
	"chain",
] as const;

/**
 * How a chain is rigged, and the end fittings each way stores. A hoist at one end hangs the chain
 * from a steelflex loop at the other; a plain chain is hooked directly at both.
 */
export const CHAIN_MODES: readonly {
	value: ChainMode;
	label: string;
	top: ChainTopEnd;
	bottom: ChainBottomEnd;
}[] = [
	{ value: "plain", label: "Plain chain", top: "direct", bottom: "direct" },
	{ value: "motor_top", label: "Motor on top", top: "motor", bottom: "steelflex_loop" },
	{
		value: "motor_bottom",
		label: "Motor on bottom",
		top: "steelflex_loop",
		bottom: "motor",
	},
];

const SRGB = /^#[0-9a-fA-F]{6}$/;

export function sceneryOptionsOf(fixture: PatchedFixture): SceneryOptions {
	return fixture.scenery_options ?? {};
}

export function isChain(fixture: PatchedFixture) {
	return sceneryOf(fixture)?.kind === "chain";
}

/**
 * How a chain is rigged, read from the hoist: one at the top or the bottom decides it, whatever the
 * other end stored. A chain placed before the choice existed has no top and hangs from a hoist.
 */
export function chainModeOf(fixture: PatchedFixture): ChainMode {
	const { chain_top, chain_bottom } = sceneryOptionsOf(fixture);
	if ((chain_top ?? "motor") === "motor") return "motor_top";
	return chain_bottom === "motor" ? "motor_bottom" : "plain";
}

export function chainModeLabel(mode: ChainMode) {
	return CHAIN_MODES.find((option) => option.value === mode)?.label ?? mode;
}

export function isSceneryOptionEdit(edit: string | null): edit is SceneryOptionEdit {
	return edit === "scenery_colour" || edit === "chain";
}

/**
 * The options after one choice, or why it is refused.
 *
 * An empty colour returns the object to its kind's own material, which is what it was drawn in
 * before anyone chose.
 */
export function sceneryOptionChange(
	fixture: PatchedFixture,
	edit: SceneryOptionEdit,
	value: string,
): { options: SceneryOptions } | { error: string } {
	const current = sceneryOptionsOf(fixture);
	if (edit === "scenery_colour") {
		const colour = value.trim();
		if (!colour) return { options: { ...current, colour_srgb: null } };
		if (!SRGB.test(colour))
			return { error: "Enter a colour as #RRGGBB, or clear it for the default." };
		return { options: { ...current, colour_srgb: colour.toUpperCase() } };
	}
	// Both ends are always written, so the stored fittings can never disagree with the mode.
	const mode = CHAIN_MODES.find((option) => option.value === value);
	return mode
		? { options: { ...current, chain_top: mode.top, chain_bottom: mode.bottom } }
		: { error: "Choose Plain chain, Motor on top or Motor on bottom." };
}
