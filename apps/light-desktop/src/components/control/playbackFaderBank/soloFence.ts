import type { PlaybackSoloRegion } from "../../../features/playbackSolo/contracts";
import type { PlaybackSlotProjection } from "./types";

/** Edges follow displayed row coordinates, never arithmetic Playback numbering. */
export function soloFenceClasses(
	cell: PlaybackSlotProjection,
	cells: readonly PlaybackSlotProjection[],
	page: number,
	columns: number,
	regions: readonly PlaybackSoloRegion[],
) {
	const memberships = regions.filter((region) =>
		region.members.some(
			(member) => member.page === page && member.position === cell.slot,
		),
	);
	if (!memberships.length) return "";
	const coordinate = (candidate: PlaybackSlotProjection) => ({
		row: candidate.footprint
			? candidate.footprint.rowStart - 1
			: candidate.rowIndex,
		column: candidate.footprint
			? candidate.footprint.columnStart - 1
			: candidate.columnIndex,
	});
	const origin = coordinate(cell);
	const neighbour = (row: number, column: number) => {
		const adjacent = cells.find((candidate) => {
			const point = coordinate(candidate);
			return point.row === row && point.column === column;
		});
		return (
			adjacent &&
			memberships.some((region) =>
				region.members.some(
					(member) => member.page === page && member.position === adjacent.slot,
				),
			)
		);
	};
	const top = Boolean(neighbour(origin.row - 1, origin.column));
	const right = Boolean(neighbour(origin.row, origin.column + 1));
	const bottom = Boolean(neighbour(origin.row + 1, origin.column));
	const left = Boolean(neighbour(origin.row, origin.column - 1));
	const classes = ["playback-solo-member"];
	for (const [edge, joined] of Object.entries({ top, right, bottom, left }))
		if (!joined) classes.push(`solo-fence-${edge}`);
	for (const [corner, joined, row, column] of [
		["top-left", top && left, origin.row - 1, origin.column - 1],
		["top-right", top && right, origin.row - 1, origin.column + 1],
		["bottom-right", bottom && right, origin.row + 1, origin.column + 1],
		["bottom-left", bottom && left, origin.row + 1, origin.column - 1],
	] as const)
		if (joined && !neighbour(row, column))
			classes.push(`solo-corner-${corner}`);
	return classes.join(" ");
}
