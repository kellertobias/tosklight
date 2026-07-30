import type {
	SelectionGridConfiguration,
	SelectionGridMethod,
} from "../api/types";
import type { StagePosition3d } from "../features/server/contracts";

export interface SelectionGridCell {
	fixtureId: string;
	row: number;
	column: number;
}

export interface SelectionGridPositions {
	positions2d: Readonly<
		Record<string, { x: number; y: number; rotation?: number }>
	>;
	positions3d: Readonly<Record<string, StagePosition3d>>;
}

type Projection = {
	fixtureId: string;
	horizontal: number;
	vertical: number;
};

export const DEFAULT_SELECTION_GRID: SelectionGridConfiguration = {
	method: "stage2d",
	axis_origin: { x: 0, y: 0, z: 0 },
};

export const SELECTION_GRID_METHODS: readonly SelectionGridMethod[] = [
	"stage2d",
	"top_to_bottom",
	"bottom_to_top",
	"front_to_back",
	"back_to_front",
	"left_to_right",
	"right_to_left",
	"horizontal_axis_x",
	"vertical_axis_z",
	"room_depth_axis_y",
];

/**
 * Derives cells without touching the Group's ordered fixture sequence.
 *
 * Exact projected positions expand horizontally by stable fixture identity. Missing positions
 * remain selectable in a deterministic overflow row.
 */
export function selectionGridCells(
	fixtureIds: readonly string[],
	configuration: SelectionGridConfiguration | undefined,
	positions: SelectionGridPositions,
): SelectionGridCell[] {
	const config = configuration ?? DEFAULT_SELECTION_GRID;
	const projected: Projection[] = [];
	const missing: string[] = [];
	for (const fixtureId of fixtureIds) {
		const projection = projectFixture(fixtureId, config, positions);
		if (projection) projected.push(projection);
		else missing.push(fixtureId);
	}
	const horizontalValues = uniqueSorted(
		projected.map((item) => item.horizontal),
		(left, right) => left - right,
	);
	const verticalValues = uniqueSorted(
		projected.map((item) => item.vertical),
		(left, right) => right - left,
	);
	const ties = new Map<number, Map<number, string[]>>();
	for (const item of projected) {
		const columnRank = horizontalValues.indexOf(item.horizontal);
		const rowRank = verticalValues.indexOf(item.vertical);
		const rows = ties.get(columnRank) ?? new Map<number, string[]>();
		const fixtures = rows.get(rowRank) ?? [];
		fixtures.push(item.fixtureId);
		rows.set(rowRank, fixtures);
		ties.set(columnRank, rows);
	}
	for (const rows of ties.values())
		for (const fixtures of rows.values()) fixtures.sort();
	const widths = horizontalValues.map((_, rank) =>
		Math.max(1, ...[...(ties.get(rank)?.values() ?? [])].map((ids) => ids.length)),
	);
	const offsets: number[] = [];
	let offset = 0;
	for (const width of widths) {
		offsets.push(offset);
		offset += width;
	}
	const cells: SelectionGridCell[] = [];
	for (const [columnRank, rows] of ties)
		for (const [row, fixtures] of rows)
			fixtures.forEach((fixtureId, tieRank) =>
				cells.push({
					fixtureId,
					row,
					column: offsets[columnRank] + tieRank,
				}),
			);
	missing.sort().forEach((fixtureId, column) =>
		cells.push({ fixtureId, row: verticalValues.length, column }),
	);
	return cells.sort(
		(left, right) =>
			left.row - right.row ||
			left.column - right.column ||
			left.fixtureId.localeCompare(right.fixtureId),
	);
}

export function rowsFirst(
	cells: readonly SelectionGridCell[],
	traversal: "top_left" | "top_right" | "bottom_left" | "bottom_right",
) {
	const top = traversal.startsWith("top");
	const left = traversal.endsWith("left");
	return [...cells]
		.sort(
			(a, b) =>
				(top ? a.row - b.row : b.row - a.row) ||
				(left ? a.column - b.column : b.column - a.column),
		)
		.map((cell) => cell.fixtureId);
}

export function columnsFirst(
	cells: readonly SelectionGridCell[],
	traversal: "top_left" | "bottom_left" | "top_right" | "bottom_right",
) {
	const left = traversal.endsWith("left");
	const top = traversal.startsWith("top");
	return [...cells]
		.sort(
			(a, b) =>
				(left ? a.column - b.column : b.column - a.column) ||
				(top ? a.row - b.row : b.row - a.row),
		)
		.map((cell) => cell.fixtureId);
}

function projectFixture(
	fixtureId: string,
	configuration: SelectionGridConfiguration,
	positions: SelectionGridPositions,
): Projection | null {
	if (configuration.method === "stage2d") {
		const position = positions.positions2d[fixtureId];
		if (!position || !finite(position.x, position.y)) return null;
		return {
			fixtureId,
			horizontal: position.x,
			vertical: -position.y,
		};
	}
	const position = positions.positions3d[fixtureId];
	if (!position || !finite(position.x, position.y, position.z)) return null;
	const origin = configuration.axis_origin ?? DEFAULT_SELECTION_GRID.axis_origin!;
	if (!finite(origin.x, origin.y, origin.z)) return null;
	const x = position.x - origin.x;
	const y = position.y - origin.y;
	const z = position.z - origin.z;
	const [horizontal, vertical] = project3d(configuration.method, x, y, z);
	return { fixtureId, horizontal, vertical };
}

function project3d(
	method: Exclude<SelectionGridMethod, "stage2d">,
	x: number,
	y: number,
	z: number,
): [number, number] {
	switch (method) {
		case "top_to_bottom":
			return [x, -y];
		case "bottom_to_top":
			return [-x, -y];
		case "front_to_back":
			return [x, z];
		case "back_to_front":
			return [-x, z];
		case "left_to_right":
			return [y, z];
		case "right_to_left":
			return [-y, z];
		case "horizontal_axis_x":
			return [x, Math.atan2(z, y)];
		case "vertical_axis_z":
			return [Math.atan2(y, x), z];
		case "room_depth_axis_y":
			return [Math.atan2(z, x), -y];
	}
}

function uniqueSorted(values: number[], compare: (a: number, b: number) => number) {
	return [...new Set(values)].sort(compare);
}

function finite(...values: number[]) {
	return values.every(Number.isFinite);
}
