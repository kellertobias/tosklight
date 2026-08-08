import type { Position3d } from "./contracts";

/**
 * The lamps drawn behind a projection so the operator can see what the shape is being placed
 * against.
 *
 * A selection is what the operator is working on, so it is what gets drawn. With nothing
 * selected the whole rig is drawn instead, which is what tells them where the shape wants to go
 * in the first place.
 */

export interface StageFixtureDot {
	id: string;
	position: Position3d;
	selected: boolean;
}

/**
 * Past this many the dots stop reading as places and start costing frames, so the rest are
 * dropped rather than drawn.
 */
export const STAGE_FIXTURE_DOT_LIMIT = 200;

interface StagePositions {
	positions: Record<string, { x: number; y: number }>;
	positions3d: Record<string, { x: number; y: number; z: number }>;
}

/** A patched fixture without a 3D position still has its 2D one, at floor level. */
function positionOf(
	id: string,
	{ positions, positions3d }: StagePositions,
): Position3d | null {
	const placed = positions3d[id];
	if (placed) return { x: placed.x, y: placed.y, z: placed.z };
	const flat = positions[id];
	return flat ? { x: flat.x, y: flat.y, z: 0 } : null;
}

export function stageFixtureDots(
	selection: readonly string[],
	layout: StagePositions,
	limit = STAGE_FIXTURE_DOT_LIMIT,
): StageFixtureDot[] {
	const selected = selection.length > 0;
	const ids = selected
		? selection
		: [
				...new Set([
					...Object.keys(layout.positions3d),
					...Object.keys(layout.positions),
				]),
			].sort((left, right) => left.localeCompare(right, undefined, { numeric: true }));
	const dots: StageFixtureDot[] = [];
	for (const id of ids) {
		if (dots.length >= limit) break;
		const position = positionOf(id, layout);
		if (position) dots.push({ id, position, selected });
	}
	return dots;
}
