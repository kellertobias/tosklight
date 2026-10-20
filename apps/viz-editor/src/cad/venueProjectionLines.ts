/** Native exterior edges already carry physical depth; semantic frames receive a face offset. */
import {
	modelDepth,
	type PlanLine,
	type PlanPoint,
	projectModelPoint,
	rotateModelPoint,
} from "./projection";
import type { CadEntity, CadViewDirection } from "./types";
type Point = [number, number, number];
export function projectVenueLines(
	entity: CadEntity,
	view: CadViewDirection,
	lines: readonly [Point, Point][],
	faceZ = 0,
): PlanLine[] {
	return lines.map((line) => {
		const points = line.map(([x, y, z]) =>
			rotateModelPoint([x, y, z + faceZ], entity.rotationDegrees),
		);
		return {
			points: points.map((point) => projectModelPoint(point, view)) as [
				PlanPoint,
				PlanPoint,
			],
			depths: points.map((point) => modelDepth(point, view)) as [
				number,
				number,
			],
		};
	});
}
