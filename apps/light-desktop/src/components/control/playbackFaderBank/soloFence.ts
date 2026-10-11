import type { PlaybackSoloRegion } from "../../../features/playbackSolo/contracts";
import { playbackCellKey } from "./projection";
import type { PlaybackSlotProjection } from "./types";

export interface SoloFenceCell {
	key: string;
	position: number;
	rect: { left: number; top: number; right: number; bottom: number };
}
export interface SoloFenceSegment {
	cellKey: string;
	edge: "top" | "right" | "bottom" | "left";
	x1: number;
	y1: number;
	x2: number;
	y2: number;
}

/** Subtract shared boundaries in the supplied coordinate space (grid units or measured pixels). */
export function soloFenceSegments(
	cells: readonly SoloFenceCell[],
	page: number,
	regions: readonly PlaybackSoloRegion[],
): SoloFenceSegment[] {
	const memberships = new Map(
		cells.map((cell) => [
			cell.key,
			new Set(
				regions
					.filter((region) =>
						region.members.some(
							(member) =>
								member.page === page && member.position === cell.position,
						),
					)
					.map((region) => region.id),
			),
		]),
	);
	return cells.flatMap((cell) => {
		const own = memberships.get(cell.key)!;
		if (!own.size) return [];
		const peers = cells.filter(
			(peer) =>
				peer.key !== cell.key &&
				[...memberships.get(peer.key)!].some((id) => own.has(id)),
		);
		return perimeter(cell, peers);
	});
}

function perimeter(cell: SoloFenceCell, peers: readonly SoloFenceCell[]) {
	const { left, top, right, bottom } = cell.rect;
	return (["top", "right", "bottom", "left"] as const).flatMap((edge) => {
		const horizontal = edge === "top" || edge === "bottom";
		const fixed =
			edge === "top"
				? top
				: edge === "bottom"
					? bottom
					: edge === "left"
						? left
						: right;
		const from = horizontal ? left : top;
		const to = horizontal ? right : bottom;
		const cuts = peers
			.flatMap((peer) => {
				const other = peer.rect;
				const boundary =
					edge === "top"
						? other.bottom
						: edge === "bottom"
							? other.top
							: edge === "left"
								? other.right
								: other.left;
				if (Math.abs(boundary - fixed) > 0.01) return [];
				const start = Math.max(from, horizontal ? other.left : other.top);
				const end = Math.min(to, horizontal ? other.right : other.bottom);
				return end > start ? [[start, end] as const] : [];
			})
			.sort((a, b) => a[0] - b[0]);
		let cursor = from;
		const exposed: [number, number][] = [];
		for (const [start, end] of cuts) {
			if (start > cursor) exposed.push([cursor, start]);
			cursor = Math.max(cursor, end);
		}
		if (cursor < to) exposed.push([cursor, to]);
		return exposed.map(
			([start, end]): SoloFenceSegment => ({
				cellKey: cell.key,
				edge,
				x1: horizontal ? start : fixed,
				y1: horizontal ? fixed : start,
				x2: horizontal ? end : fixed,
				y2: horizontal ? fixed : end,
			}),
		);
	});
}

export function soloFenceCell(cell: PlaybackSlotProjection): SoloFenceCell {
	const footprint = cell.footprint;
	const left = footprint ? footprint.columnStart - 1 : cell.columnIndex;
	const top = footprint ? footprint.rowStart - 1 : cell.rowIndex;
	return {
		key: playbackCellKey(cell),
		position: cell.slot,
		rect: {
			left,
			top,
			right: left + (footprint?.columnSpan ?? 1),
			bottom: top + (footprint?.rowSpan ?? 1),
		},
	};
}

/** Classes describe which parts are exposed; SVG segments draw the actual partial perimeter. */
export function soloFenceClasses(
	cell: PlaybackSlotProjection,
	cells: readonly PlaybackSlotProjection[],
	page: number,
	_columns: number,
	regions: readonly PlaybackSoloRegion[],
) {
	const memberships = regions.filter((region) =>
		region.members.some(
			(member) => member.page === page && member.position === cell.slot,
		),
	);
	if (!memberships.length) return "";
	const shape = cells.map(soloFenceCell);
	const own = soloFenceCell(cell);
	const peers = shape.filter(
		(candidate) =>
			candidate.key !== own.key &&
			memberships.some((region) =>
				region.members.some(
					(member) =>
						member.page === page && member.position === candidate.position,
				),
			),
	);
	const segments = perimeter(own, peers);
	const edges = new Set(segments.map((segment) => segment.edge));
	const classes = [
		"playback-solo-member",
		...[...edges].map((edge) => `solo-fence-${edge}`),
	];
	const neighbour = (row: number, column: number) =>
		shape.some(
			(candidate) =>
				candidate.rect.left <= column &&
				candidate.rect.right > column &&
				candidate.rect.top <= row &&
				candidate.rect.bottom > row &&
				memberships.some((region) =>
					region.members.some(
						(member) =>
							member.page === page && member.position === candidate.position,
					),
				),
		);
	const { top, left, right, bottom } = own.rect;
	for (const [corner, a, b, row, column] of [
		["top-left", "top", "left", top - 1, left - 1],
		["top-right", "top", "right", top - 1, right],
		["bottom-right", "bottom", "right", bottom, right],
		["bottom-left", "bottom", "left", bottom, left - 1],
	] as const)
		if (!edges.has(a) && !edges.has(b) && !neighbour(row, column))
			classes.push(`solo-corner-${corner}`);
	return classes.join(" ");
}

export function soloFenceClassMap(
	cells: readonly PlaybackSlotProjection[],
	page: number | null,
	columns: number,
	regions: readonly PlaybackSoloRegion[],
) {
	return new Map(
		cells.map((cell) => [
			playbackCellKey(cell),
			page == null ? "" : soloFenceClasses(cell, cells, page, columns, regions),
		]),
	);
}
