import type { CSSProperties } from "react";

export type PoolObjectType =
	| "group"
	| "preset"
	| "cuelist"
	| "sequence"
	| "dynamic"
	| "macro";

export type PoolColorMode = "type" | "individual";

export type PoolPresetFamily =
	| "mixed"
	| "intensity"
	| "color"
	| "position"
	| "beam";

export interface PoolColorPalette {
	group: string;
	macro: string;
	dynamic: string;
	cuelist: string;
	sequence: string;
	preset: Record<PoolPresetFamily, string>;
}

export type PoolPresentationState =
	| "empty"
	| "selected"
	| "focused"
	| "active"
	| "disabled"
	| "store-target"
	| "record-target"
	| "update-target"
	| "set-target"
	| "copy-target"
	| "move-target"
	| "delete-target";

export const DEFAULT_POOL_COLOR_PALETTE: Readonly<PoolColorPalette> = {
	group: "#d8ad55",
	macro: "#8f3541",
	dynamic: "#3bbdce",
	cuelist: "#93cc55",
	sequence: "#93cc55",
	preset: {
		mixed: "#89939e",
		intensity: "#89939e",
		color: "#89939e",
		position: "#89939e",
		beam: "#89939e",
	},
};

export const INDIVIDUAL_POOL_COLOR_FALLBACK = "#89939e";

export interface PoolPresentationInput {
	objectType: PoolObjectType;
	presetFamily?: PoolPresetFamily;
	mode: PoolColorMode;
	itemColor?: string | null;
	palette?: PoolColorPalette;
	states?: readonly PoolPresentationState[];
}

export interface ResolvedPoolPresentation {
	color: string;
	states: PoolPresentationState[];
	className: string;
	style: CSSProperties;
}

/**
 * Resolves the shared visual language for every pool surface. Workflow and
 * interaction states remain explicit classes so they never depend on color.
 */
export function resolvePoolPresentation({
	objectType,
	presetFamily = "mixed",
	mode,
	itemColor,
	palette = DEFAULT_POOL_COLOR_PALETTE,
	states = [],
}: PoolPresentationInput): ResolvedPoolPresentation {
	const color =
		mode === "individual"
			? (normalizeColor(itemColor) ?? INDIVIDUAL_POOL_COLOR_FALLBACK)
			: objectType === "preset"
				? palette.preset[presetFamily]
				: palette[objectType];
	const normalizedStates = [...new Set(states)];
	const empty = normalizedStates.includes("empty");
	return {
		color,
		states: normalizedStates,
		className: [
			"pool-presentation",
			`pool-type-${objectType}`,
			`pool-color-mode-${mode}`,
			...normalizedStates,
		].join(" "),
		style: (empty ? {} : { "--pool-card-color": color }) as CSSProperties,
	};
}

function normalizeColor(color: string | null | undefined) {
	const trimmed = color?.trim();
	return trimmed ? trimmed : null;
}
