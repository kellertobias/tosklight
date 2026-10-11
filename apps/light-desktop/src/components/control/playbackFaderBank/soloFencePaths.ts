import type { SoloFenceSegment } from "./soloFence";

type Point = { x: number; y: number };
type Edge = { from: Point; to: Point };
const pointKey = (point: Point) =>
	`${point.x.toFixed(2)}:${point.y.toFixed(2)}`;
const distance = (a: Point, b: Point) => Math.hypot(b.x - a.x, b.y - a.y);

function directedEdge(line: SoloFenceSegment): Edge {
	const a = { x: line.x1, y: line.y1 };
	const b = { x: line.x2, y: line.y2 };
	return line.edge === "bottom" || line.edge === "left"
		? { from: b, to: a }
		: { from: a, to: b };
}
function turn(previous: Edge, next: Edge) {
	const ax = previous.to.x - previous.from.x;
	const ay = previous.to.y - previous.from.y;
	const bx = next.to.x - next.from.x;
	const by = next.to.y - next.from.y;
	return Math.atan2(ax * by - ay * bx, ax * bx + ay * by);
}
/** Follow exposed clockwise edges. A right turn keeps corner-touching islands separate. */
function contours(lines: readonly SoloFenceSegment[]): Point[][] {
	const remaining = new Set(lines.map(directedEdge));
	const outgoing = new Map<string, Edge[]>();
	for (const edge of remaining) {
		const key = pointKey(edge.from);
		outgoing.set(key, [...(outgoing.get(key) ?? []), edge]);
	}
	const paths: Point[][] = [];
	while (remaining.size) {
		const first = remaining.values().next().value!;
		let edge = first;
		const points = [first.from];
		while (remaining.delete(edge)) {
			points.push(edge.to);
			if (pointKey(edge.to) === pointKey(first.from)) break;
			const candidates = (outgoing.get(pointKey(edge.to)) ?? []).filter(
				(candidate) => remaining.has(candidate),
			);
			candidates.sort((a, b) => turn(edge, b) - turn(edge, a));
			if (!candidates.length) break;
			edge = candidates[0];
		}
		if (pointKey(points.at(-1)!) === pointKey(points[0]))
			paths.push(points.slice(0, -1));
	}
	return paths;
}
function corners(points: readonly Point[]) {
	return points.filter((point, index) => {
		const before = points[(index + points.length - 1) % points.length];
		const after = points[(index + 1) % points.length];
		return (
			Math.abs(
				(point.x - before.x) * (after.y - point.y) -
					(point.y - before.y) * (after.x - point.x),
			) > 0.01
		);
	});
}
function inset(point: Point, toward: Point, radius: number): Point {
	const fraction = radius / distance(point, toward);
	return {
		x: point.x + (toward.x - point.x) * fraction,
		y: point.y + (toward.y - point.y) * fraction,
	};
}
const coordinate = (point: Point) => `${point.x} ${point.y}`;
function roundedPath(points: readonly Point[], radius: number) {
	const vertices = corners(points);
	if (vertices.length < 3) return "";
	const rounded = vertices.map((point, index) => {
		const before = vertices[(index + vertices.length - 1) % vertices.length];
		const after = vertices[(index + 1) % vertices.length];
		const amount = Math.min(
			radius,
			distance(before, point) / 2,
			distance(point, after) / 2,
		);
		return {
			point,
			entry: inset(point, before, amount),
			exit: inset(point, after, amount),
		};
	});
	return `M ${coordinate(rounded[0].entry)} ${rounded
		.map(
			(corner, index) =>
				`${index ? `L ${coordinate(corner.entry)} ` : ""}Q ${coordinate(corner.point)} ${coordinate(corner.exit)}`,
		)
		.join(" ")} Z`;
}
/** One rounded closed perimeter per connected island, preserving concave boundaries. */
export function soloFencePaths(
	lines: readonly SoloFenceSegment[],
	radius = 6,
): string[] {
	return contours(lines)
		.map((points) => roundedPath(points, radius))
		.filter(Boolean);
}
