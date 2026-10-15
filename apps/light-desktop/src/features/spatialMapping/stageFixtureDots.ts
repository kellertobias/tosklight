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

/** Server-resolved physical positions; the UI never reconstructs the fixture geometry. */
interface PhysicalFixturePosition {
	fixture_id: string;
	physical_fixture_id: string;
	position: Position3d;
}

export function stageFixtureDots(
	selection: readonly string[],
	positions: readonly PhysicalFixturePosition[],
	limit = STAGE_FIXTURE_DOT_LIMIT,
): StageFixtureDot[] {
	const byId = new Map(positions.map((row) => [row.fixture_id, row]));
	const selected = selection.length > 0;
	const ids = selected
		? [...new Set(selection)]
		: positions
				.filter((row) => row.fixture_id === row.physical_fixture_id)
				.map((row) => row.fixture_id)
				.sort((left, right) =>
					left.localeCompare(right, undefined, { numeric: true }),
				);
	const dots: StageFixtureDot[] = [];
	for (const id of ids) {
		if (dots.length >= limit) break;
		const row = byId.get(id);
		if (row && Object.values(row.position).every(Number.isFinite))
			dots.push({ id, position: row.position, selected });
	}
	return dots;
}
