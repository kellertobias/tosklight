/**
 * Hidden-line removal for drawn symbols, the way a technical drawing leaves out what is behind.
 *
 * A symbol is a list of parts in painter's order, back to front. Each solid part has the edges it
 * draws and the convex pieces of area it covers; a later part's area hides every earlier edge and
 * line that falls inside it or on its boundary — its own edge is drawn there already. The result is
 * decided once, in the geometry, so the screen and a printed PDF show the same drawing whatever
 * the renderer's depth test does with shapes that all sit at one depth.
 */
import type { PlanLine, PlanPoint } from "./projection";

export type DrawnPart =
	| {
			kind: "solid";
			/** Closed loops the part draws. */
			edges: PlanPoint[][];
			/** Convex pieces of the area the part covers; empty for a part that covers nothing. */
			area: PlanPoint[][];
	  }
	| { kind: "line"; line: PlanLine };

/** How close to a boundary still counts as on it, in millimetres. */
const TOLERANCE = 1e-5;
/** Visible pieces shorter than this are left out. */
const SHORTEST = 0.05;

interface Piece {
	points: PlanPoint[];
	orientation: number;
	min: PlanPoint;
	max: PlanPoint;
}

/**
 * The drawn edges of `parts` with every hidden stretch removed: loops nothing covers stay whole
 * `outlines`; loops something covers, and every open line, become their visible `lines`.
 */
export function hideCoveredEdges(parts: readonly DrawnPart[]): {
	outlines: PlanPoint[][];
	lines: PlanLine[];
} {
	const pieces: Piece[][] = parts.map((part) =>
		part.kind === "solid" ? part.area.filter((area) => area.length >= 3).map(piece) : [],
	);
	const outlines: PlanPoint[][] = [];
	const lines: PlanLine[] = [];
	parts.forEach((part, index) => {
		const covers = pieces.slice(index + 1).flat();
		if (part.kind === "line") {
			lines.push(...visible(part.line.points[0], part.line.points[1], covers));
			return;
		}
		for (const loop of part.edges) {
			const segments = loop.map((point, at) => visible(point, loop[(at + 1) % loop.length], covers));
			const whole = segments.every(
				(pieces, at) =>
					pieces.length === 1 &&
					samePoint(pieces[0].points[0], loop[at]) &&
					samePoint(pieces[0].points[1], loop[(at + 1) % loop.length]),
			);
			if (whole) outlines.push(loop);
			else lines.push(...segments.flat());
		}
	});
	return { outlines, lines };
}

function piece(points: PlanPoint[]): Piece {
	let area = 0;
	points.forEach(([x, y], index) => {
		const [nx, ny] = points[(index + 1) % points.length];
		area += x * ny - nx * y;
	});
	const xs = points.map(([x]) => x);
	const ys = points.map(([, y]) => y);
	return {
		points,
		orientation: Math.sign(area) || 1,
		min: [Math.min(...xs), Math.min(...ys)],
		max: [Math.max(...xs), Math.max(...ys)],
	};
}

/** The stretches of the segment `a`–`b` that no piece covers. */
function visible(a: PlanPoint, b: PlanPoint, covers: readonly Piece[]): PlanLine[] {
	const hidden: [number, number][] = [];
	for (const cover of covers) {
		if (
			Math.max(a[0], b[0]) < cover.min[0] - TOLERANCE ||
			Math.min(a[0], b[0]) > cover.max[0] + TOLERANCE ||
			Math.max(a[1], b[1]) < cover.min[1] - TOLERANCE ||
			Math.min(a[1], b[1]) > cover.max[1] + TOLERANCE
		)
			continue;
		const span = coveredSpan(a, b, cover);
		if (span) hidden.push(span);
	}
	hidden.sort((left, right) => left[0] - right[0]);
	const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
	const result: PlanLine[] = [];
	let from = 0;
	const emit = (to: number) => {
		if ((to - from) * length > SHORTEST) result.push({ points: [at(a, b, from), at(a, b, to)] });
	};
	for (const [start, end] of hidden) {
		if (start > from) emit(start);
		from = Math.max(from, end);
		if (from >= 1) break;
	}
	if (from < 1) emit(1);
	return result;
}

/** The part of the segment `a`–`b`, as parameters from 0 to 1, inside or on a convex piece. */
function coveredSpan(a: PlanPoint, b: PlanPoint, cover: Piece): [number, number] | null {
	let low = 0;
	let high = 1;
	const { points, orientation } = cover;
	for (let index = 0; index < points.length; index++) {
		const p = points[index];
		const q = points[(index + 1) % points.length];
		const ex = q[0] - p[0];
		const ey = q[1] - p[1];
		const edge = Math.hypot(ex, ey);
		if (edge < 1e-9) continue;
		// Signed distance from the edge's line, positive inside, as the segment runs from a to b.
		const start = (orientation * (ex * (a[1] - p[1]) - ey * (a[0] - p[0]))) / edge + TOLERANCE;
		const change = (orientation * (ex * (b[1] - a[1]) - ey * (b[0] - a[0]))) / edge;
		if (Math.abs(change) < 1e-12) {
			if (start < 0) return null;
			continue;
		}
		const t = -start / change;
		if (change > 0) low = Math.max(low, t);
		else high = Math.min(high, t);
		if (low > high) return null;
	}
	return high - low > 1e-9 ? [low, high] : null;
}

function at(a: PlanPoint, b: PlanPoint, t: number): PlanPoint {
	return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
}

function samePoint(a: PlanPoint, b: PlanPoint): boolean {
	return Math.abs(a[0] - b[0]) < 1e-9 && Math.abs(a[1] - b[1]) < 1e-9;
}
