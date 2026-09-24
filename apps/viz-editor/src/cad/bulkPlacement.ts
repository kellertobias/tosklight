/**
 * Where each element of a Place Multiple stands: a grid of stage elements, or a run of truss.
 *
 * A rig is rarely one of anything. A stage is a field of identical decks butted edge to edge, and a
 * truss rig is a run of sections from one point to another. Both are laid out here, in metres in
 * plan axes — X across, Y deep, Z up — and returned as whole millimetres, one placement per element
 * in the order they are made.
 *
 * A stage element is placed by the middle of its footprint on the floor, and a truss by the centre
 * of its box, so a grid is centred on the point asked for and a run's sections are centred along
 * the line between its two ends.
 */

export interface PlanPlacement {
	position: { x: number; y: number; z: number };
	rotation: { x: number; y: number; z: number };
}

/** A point in metres, in plan axes. */
export interface MetrePoint {
	x: number;
	y: number;
	z: number;
}

/** Whole millimetres, with a rounded -0 turned back into 0. */
function millimetres(metres: number): number {
	return Math.round(metres * 1000) + 0;
}

export interface StageGrid {
	/** How many elements across (X), at least one. */
	columns: number;
	/** How many elements deep (Y), at least one. */
	rows: number;
	/** Whether each element is turned a quarter turn, so its long side runs deep instead of across. */
	turned: boolean;
	/** The element's own footprint in metres, as its profile is built, before it is turned. */
	footprint: { width: number; depth: number };
	/** Where the middle of the whole grid stands, in metres; its Z is the height the decks are placed at. */
	centre?: MetrePoint;
}

/**
 * A field of stage elements butted edge to edge, row by row, centred on the grid's centre.
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
	const centre = grid.centre ?? { x: 0, y: 0, z: 0 };
	const startX = centre.x - ((columns - 1) * stepX) / 2;
	const startY = centre.y - ((rows - 1) * stepY) / 2;
	const placements: PlanPlacement[] = [];
	for (let row = 0; row < rows; row += 1)
		for (let column = 0; column < columns; column += 1)
			placements.push({
				position: {
					x: millimetres(startX + column * stepX),
					y: millimetres(startY + row * stepY),
					z: millimetres(centre.z),
				},
				rotation: { x: 0, y: 0, z: grid.turned ? 90 : 0 },
			});
	return placements;
}

export interface TrussRun {
	/** Where the run starts, in metres. */
	first: MetrePoint;
	/** Where the run ends, in metres. */
	last: MetrePoint;
	/** How many sections the run is made of, at least one. */
	count: number;
}

/** How long each section of a run is when the sections fill it end to end, in metres, on the plan. */
export function trussRunSectionLength(run: TrussRun): number {
	const count = Math.max(1, Math.floor(run.count));
	return Math.hypot(run.last.x - run.first.x, run.last.y - run.first.y) / count;
}

/**
 * A run of truss sections from one point to another, evenly spaced: section `i` of `n` is centred
 * `(i + ½) / n` of the way along, so `n` sections of the run's length over `n` fill it end to end.
 *
 * Each section is turned about Z only, to the run's heading on the plan — a truss is never pitched
 * or rolled here — and when the ends stand at different heights each section is raised to the
 * height of its own place along the run.
 */
export function trussRunPlacements(run: TrussRun): PlanPlacement[] {
	const count = Math.max(1, Math.floor(run.count));
	const dx = run.last.x - run.first.x;
	const dy = run.last.y - run.first.y;
	// A run with no length on the plan keeps the truss's own heading.
	const heading = Math.hypot(dx, dy) > 1e-9 ? (Math.atan2(dy, dx) * 180) / Math.PI : 0;
	const yaw = Math.round(heading * 100) / 100 + 0;
	return Array.from({ length: count }, (_, index) => {
		const along = (index + 0.5) / count;
		return {
			position: {
				x: millimetres(run.first.x + dx * along),
				y: millimetres(run.first.y + dy * along),
				z: millimetres(run.first.z + (run.last.z - run.first.z) * along),
			},
			rotation: { x: 0, y: 0, z: yaw },
		};
	});
}
