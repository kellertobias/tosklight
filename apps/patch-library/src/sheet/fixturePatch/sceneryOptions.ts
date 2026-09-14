import type { PatchedFixture } from "../../wire";
import { sceneryOf } from "./scenerySize";

export type SceneryOptions = NonNullable<PatchedFixture["scenery_options"]>;
export type ChainTopEnd = NonNullable<SceneryOptions["chain_top"]>;
export type ChainBottomEnd = NonNullable<SceneryOptions["chain_bottom"]>;
export type SceneryOptionEdit = "scenery_colour" | "chain_top" | "chain_bottom";

/** The patch sheet columns these choices are shown in, in table order. */
export const SCENERY_OPTION_COLUMNS = [
	"scenery_colour",
	"chain_top",
	"chain_bottom",
] as const;

/** What can hang at the top of a chain, as a rigger calls it. */
export const CHAIN_TOP_ENDS: readonly { value: ChainTopEnd; label: string }[] = [
	{ value: "motor", label: "Hoist" },
	{ value: "direct", label: "Direct" },
];

/** What a chain can end in at the bottom. */
export const CHAIN_BOTTOM_ENDS: readonly { value: ChainBottomEnd; label: string }[] =
	[
		{ value: "direct", label: "Direct" },
		{ value: "steelflex_loop", label: "Steelflex loop" },
	];

const SRGB = /^#[0-9a-fA-F]{6}$/;

export function sceneryOptionsOf(fixture: PatchedFixture): SceneryOptions {
	return fixture.scenery_options ?? {};
}

export function isChain(fixture: PatchedFixture) {
	return sceneryOf(fixture)?.kind === "chain";
}

/** A chain placed before its ends could be chosen hangs from a hoist by a direct hook. */
export function chainTopOf(fixture: PatchedFixture): ChainTopEnd {
	return sceneryOptionsOf(fixture).chain_top ?? "motor";
}

export function chainBottomOf(fixture: PatchedFixture): ChainBottomEnd {
	return sceneryOptionsOf(fixture).chain_bottom ?? "direct";
}

export function chainEndLabel(value: ChainTopEnd | ChainBottomEnd) {
	return (
		[...CHAIN_TOP_ENDS, ...CHAIN_BOTTOM_ENDS].find(
			(option) => option.value === value,
		)?.label ?? value
	);
}

export function isSceneryOptionEdit(edit: string | null): edit is SceneryOptionEdit {
	return edit === "scenery_colour" || edit === "chain_top" || edit === "chain_bottom";
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
	if (edit === "chain_top") {
		const end = CHAIN_TOP_ENDS.find((option) => option.value === value);
		return end
			? { options: { ...current, chain_top: end.value } }
			: { error: "Choose Hoist or Direct for the top of the chain." };
	}
	const end = CHAIN_BOTTOM_ENDS.find((option) => option.value === value);
	return end
		? { options: { ...current, chain_bottom: end.value } }
		: { error: "Choose Direct or Steelflex loop for the bottom of the chain." };
}
