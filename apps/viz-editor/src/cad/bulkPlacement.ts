/**
 * Where each element of a bulk placement stands: a deck of stage elements, or rows of truss.
 *
 * A rig is rarely one of anything. A stage is a field of identical decks butted edge to edge, and a
 * truss rig is the same run flown again at two or three heights over two or three lines of the
 * room. Both are laid out here, in metres in plan axes — X across, Y deep, Z up — and returned as
 * whole millimetres, one placement per element in the order they are made.
 *
 * A stage element is placed by the middle of its footprint on the floor, and a truss by the centre
 * of its box, which is why the two shapes count their origins differently: a deck grid grows away
 * from the origin so its first deck lands exactly where one pressed on its own would, and a truss
 * row stands at the height it is flown.
 */

export interface PlanPlacement {
	position: { x: number; y: number; z: number };
	rotation: { x: number; y: number; z: number };
}

/** Whole millimetres, with a rounded -0 turned back into 0. */
function millimetres(metres: number): number {
	return Math.round(metres * 1000) + 0;
}

export interface StageGrid {
	/** How many elements across, at least one. */
	columns: number;
	/** How many elements deep, at least one. */
	rows: number;
	/** Whether each element is turned a quarter turn, so its long side runs deep instead of across. */
	turned: boolean;
	/** The element's own footprint in metres, as its profile is built, before it is turned. */
	footprint: { width: number; depth: number };
}

/**
 * A field of stage elements butted edge to edge, row by row from the origin.
 *
 * The step is the element's own footprint rather than a spacing the operator types, because a
 * stage with a gap in it is not a stage. Turning the elements turns the step with them, so a
 * 2 × 1 m deck on its side steps 1 m across and 2 m deep.
 */
export function stageGridPlacements(grid: StageGrid): PlanPlacement[] {
	const columns = Math.max(1, Math.floor(grid.columns));
	const rows = Math.max(1, Math.floor(grid.rows));
	const { width, depth } = grid.footprint;
	const [stepX, stepY] = grid.turned ? [depth, width] : [width, depth];
	const placements: PlanPlacement[] = [];
	for (let row = 0; row < rows; row += 1)
		for (let column = 0; column < columns; column += 1)
			placements.push({
				position: { x: millimetres(column * stepX), y: millimetres(row * stepY), z: 0 },
				rotation: { x: 0, y: 0, z: grid.turned ? 90 : 0 },
			});
	return placements;
}

export interface TrussRows {
	/** The heights the runs are flown at, in metres. */
	heights: readonly number[];
	/** Where each run stands along the room, in metres. */
	positions: readonly number[];
	/** Whether the runs are turned a quarter turn, so they cross the room instead of running down it. */
	turned: boolean;
}

/**
 * One truss run per height and position: the same run flown again at each height, over each line.
 *
 * A run left as it is lies along X, so its positions are how far back each line stands — Y. Turned
 * a quarter turn it lies along Y instead, and its positions are how far across each line stands —
 * X. Heights are the outer loop, so the rows come out lowest-first, line by line.
 */
export function trussRowPlacements(rows: TrussRows): PlanPlacement[] {
	const placements: PlanPlacement[] = [];
	for (const height of rows.heights)
		for (const position of rows.positions)
			placements.push({
				position: {
					x: millimetres(rows.turned ? position : 0),
					y: millimetres(rows.turned ? 0 : position),
					z: millimetres(height),
				},
				rotation: { x: 0, y: 0, z: rows.turned ? 90 : 0 },
			});
	return placements;
}

/**
 * A typed list of metres: `4 6 8`, `4, 6, 8`, or a range `4 THRU 8 BY 2`.
 *
 * Values are separated by spaces, and a comma is a decimal point as it is in every other field on
 * the desk — so `4,5 6` is four and a half metres and then six, and a list written `4, 6, 8` still
 * reads as three values because the trailing commas fall off the ends. A run of evenly spaced
 * lines is a range rather than a list to count out. `null` when the text holds anything that is
 * none of those, so the wizard can say so rather than place half of it.
 */
export function parseMetreList(text: string): number[] | null {
	const trimmed = text.trim();
	if (!trimmed) return [];
	const range =
		/^(-?[\d.,]+)\s*(?:THRU|…|\.\.\.)\s*(-?[\d.,]+)(?:\s*BY\s*(-?[\d.,]+))?$/iu.exec(trimmed);
	if (range) {
		const [first, last] = [number(range[1]), number(range[2])];
		const by = range[3] ? number(range[3]) : 1;
		if (first == null || last == null || by == null) return null;
		const step = Math.abs(by);
		if (!(step > 0)) return null;
		const direction = last >= first ? 1 : -1;
		const count = Math.floor(Math.abs(last - first) / step + 1e-6) + 1;
		return Array.from({ length: count }, (_, index) => first + direction * step * index);
	}
	const values = trimmed.split(/\s+/u).map(number);
	return values.some((value) => value == null) ? null : (values as number[]);
}

function number(text: string): number | null {
	const value = Number(text.replace(",", "."));
	return Number.isFinite(value) ? value : null;
}
