/** Native venue vertices converted to desk space once, shared by bounds and depth slicing. */
import { rotateModelPoint } from "./projection";
import type { CadEntity } from "./types";

type VenuePlacement = Pick<
	CadEntity,
	"positionMillimetres" | "rotationDegrees"
> &
	Partial<Pick<CadEntity, "venueGeometry">>;
export function venueWorldPoints(
	entity: VenuePlacement,
	includePlan = false,
): [number, number, number][] {
	const geometry = entity.venueGeometry;
	if (!geometry) return [];
	const points = geometry.triangles.flat();
	if (includePlan || !points.length)
		points.push(
			...geometry.plan.flatMap((line) => line.map(([x, z]) => [x, 0, z])),
		);
	return points.map((point) => {
		const [x, y, z] = rotateModelPoint(
			point as [number, number, number],
			entity.rotationDegrees,
		);
		return [
			entity.positionMillimetres[0] + x,
			entity.positionMillimetres[1] - z,
			entity.positionMillimetres[2] + y,
		];
	});
}

/** A venue's visible centre may be far above or to the side of its placement datum. */
export function venueWorldCentre(
	entity: VenuePlacement,
): [number, number, number] | null {
	const points = venueWorldPoints(entity);
	if (!points.length) return null;
	const low = [Infinity, Infinity, Infinity],
		high = [-Infinity, -Infinity, -Infinity];
	for (const point of points)
		for (const axis of [0, 1, 2] as const) {
			low[axis] = Math.min(low[axis], point[axis]);
			high[axis] = Math.max(high[axis], point[axis]);
		}
	return [0, 1, 2].map((axis) => (low[axis] + high[axis]) / 2) as [
		number,
		number,
		number,
	];
}
