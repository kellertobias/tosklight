import type { CadEntity } from "./types";
import { wallEndpoints, type Vec3 } from "./venueShapes";

const distance = (a: Vec3, b: Vec3) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const cross = (a: number[], b: number[]) => a[0] * b[1] - a[1] * b[0];
const rounded = (point: Vec3) => point.map((v) => Math.round(v) + 0) as Vec3;
/** Corners win over edges; segment intersections stay near the pointer, never far along a ray. */
export function snapDrawnWall(
	point: Vec3,
	start: Vec3 | null,
	entities: readonly CadEntity[],
	threshold: number,
	closure: Vec3 | null,
): Vec3 {
	const segments = entities
		.filter((e) => e.selectable)
		.map(wallEndpoints)
		.filter(
			(ends) => ends.length === 2 && Math.abs(ends[0][2] - point[2]) <= 250,
		);
	const corners = [...segments.flat(), ...(closure ? [closure] : [])];
	const corner = corners
		.filter((end) => distance(end, point) <= threshold)
		.sort((a, b) => distance(a, point) - distance(b, point))[0];
	if (corner) return rounded(corner);
	const angle = start
		? Math.round(
				Math.atan2(point[1] - start[1], point[0] - start[0]) / (Math.PI / 4),
			) *
			(Math.PI / 4)
		: 0;
	const direction = [Math.cos(angle), Math.sin(angle)];
	let result = point;
	if (start) {
		const length =
			(point[0] - start[0]) * direction[0] +
			(point[1] - start[1]) * direction[1];
		result = [
			start[0] + length * direction[0],
			start[1] + length * direction[1],
			start[2],
		];
	}
	let nearest = threshold;
	for (const [a, b] of segments) {
		const delta = [b[0] - a[0], b[1] - a[1]];
		const lengthSquared = delta[0] ** 2 + delta[1] ** 2;
		if (lengthSquared < 1) continue;
		let fraction: number;
		if (start) {
			const determinant = cross(direction, delta);
			if (Math.abs(determinant) < 1e-8) continue;
			const offset = [a[0] - start[0], a[1] - start[1]];
			const ray = cross(offset, delta) / determinant;
			if (ray < 0) continue;
			fraction = cross(offset, direction) / determinant;
		} else
			fraction =
				((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1]) /
				lengthSquared;
		if (fraction < 0 || fraction > 1) continue;
		const candidate: Vec3 = [
			a[0] + fraction * delta[0],
			a[1] + fraction * delta[1],
			a[2],
		];
		const gap = distance(candidate, point);
		if (gap <= nearest) {
			nearest = gap;
			result = candidate;
		}
	}
	return rounded(result);
}
