import { viewDepth } from "./cutPlanes";
import type { CadEntity, CadViewDirection } from "./types";

/**
 * The axis that runs across the depth preview.
 *
 * The preview looks at the rig from the side the current view cannot see along, so its horizontal
 * axis is the depth being cut and this is what fills the other one. Looking along the rig from the
 * left, that companion is the plan; looking down, it is an elevation.
 */
export function previewLateral(
	point: readonly [number, number, number],
	view: CadViewDirection,
): number {
	switch (view) {
		// Looking down cuts height, so the preview is an elevation and the stage runs across it.
		case "top_down":
			return point[0];
		// Looking along the rig cuts one ground axis, so the preview is the plan.
		case "left_to_right":
		case "right_to_left":
			return point[1];
		case "front_to_back":
		case "back_to_front":
			return point[0];
	}
}

/** What the preview is a picture of, named for the operator rather than for the axis. */
export function previewLabel(view: CadViewDirection): string {
	return view === "top_down" ? "Elevation" : "Plan";
}

export interface PreviewMark {
	id: string;
	depth: number;
	lateral: number;
}

/** Every element as one mark in the preview, positioned by depth and its companion axis. */
export function previewMarks(
	entities: readonly CadEntity[],
	view: CadViewDirection,
): PreviewMark[] {
	return entities.map((entity) => ({
		id: entity.id,
		depth: viewDepth(entity.positionMillimetres, view),
		lateral: previewLateral(entity.positionMillimetres, view),
	}));
}

export interface PreviewBounds {
	minDepth: number;
	maxDepth: number;
	minLateral: number;
	maxLateral: number;
}

/**
 * The span the preview draws, padded so marks on the edge are not clipped in half.
 *
 * An empty drawing, or one where every element shares a depth, still needs a span the two cut
 * lines can be dragged along, so a degenerate range is opened out rather than left at zero.
 */
export function previewBounds(marks: readonly PreviewMark[]): PreviewBounds {
	if (!marks.length) {
		return { minDepth: 0, maxDepth: 1_000, minLateral: 0, maxLateral: 1_000 };
	}
	const depths = marks.map((mark) => mark.depth);
	const laterals = marks.map((mark) => mark.lateral);
	const pad = (low: number, high: number) => {
		const margin = Math.max((high - low) * 0.1, 500);
		return [low - margin, high + margin] as const;
	};
	const [minDepth, maxDepth] = pad(Math.min(...depths), Math.max(...depths));
	const [minLateral, maxLateral] = pad(
		Math.min(...laterals),
		Math.max(...laterals),
	);
	return { minDepth, maxDepth, minLateral, maxLateral };
}

/** Where a depth sits across the preview, as a fraction of its width. */
export function depthFraction(depth: number, bounds: PreviewBounds): number {
	const span = bounds.maxDepth - bounds.minDepth;
	if (span <= 0) return 0.5;
	return (depth - bounds.minDepth) / span;
}

/** The depth a fraction of the preview's width stands for, clamped to what it draws. */
export function fractionDepth(fraction: number, bounds: PreviewBounds): number {
	const clamped = Math.min(1, Math.max(0, fraction));
	return bounds.minDepth + clamped * (bounds.maxDepth - bounds.minDepth);
}
