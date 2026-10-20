/** The shared venue solids and plan symbols, including real openings and door swing arcs. */
import {
	modelDepth,
	type PlanGeometry,
	type PlanPoint,
	projectModelPoint,
	rotateModelPoint,
} from "./projection";
import type { CadDrawing, CadEntity, CadViewDirection } from "./types";
export function venuePlanGeometry(
	entity: CadEntity,
	view: CadViewDirection,
	projectMesh: (
		entity: CadEntity,
		drawing: CadDrawing,
		view: CadViewDirection,
	) => PlanGeometry | null,
): PlanGeometry {
	const geometry = entity.venueGeometry;
	if (!geometry)
		return { source: "live_model", triangles: [], outlines: [], lines: [] };
	const result = projectMesh(
		{ ...entity, venueGeometry: undefined, bracketAngle: 0 },
		{
			id: entity.id,
			projections: [],
			liveMeshes: [
				{
					pose: view === "top_down" ? "top" : "elevation",
					triangles: geometry.triangles.map((points) => ({
						pointsMillimetres: points as [
							[number, number, number],
							[number, number, number],
							[number, number, number],
						],
						colour: [0.38, 0.42, 0.47],
					})),
				},
			],
		},
		view,
	) ?? {
		source: "live_model" as const,
		triangles: [],
		outlines: [],
		lines: [],
	};
	if (view === "top_down") {
		// Physical faces preserve holes and stair treads during picking without filling the plan.
		if (
			entity.scenery?.architecture?.kind !== "wall" &&
			geometry.triangles.length
		)
			result.pickTriangles = result.triangles;
		// The shared plan deliberately leaves gaps for doors and includes their swing arcs.
		result.triangles = [];
		result.lines = geometry.plan.map((line) => ({
			points: line.map(([x, z]) =>
				projectModelPoint(
					rotateModelPoint([x, 0, z], entity.rotationDegrees),
					view,
				),
			) as [PlanPoint, PlanPoint],
		}));
	}
	if (view !== "top_down" && geometry.elevation?.length) {
		const horizontal = projectModelPoint(
			rotateModelPoint([1, 0, 0], entity.rotationDegrees),
			view,
		);
		const vertical = projectModelPoint(
			rotateModelPoint([0, 1, 0], entity.rotationDegrees),
			view,
		);
		const faceArea = horizontal[0] * vertical[1] - horizontal[1] * vertical[0];
		// Edge-on walls retain their thickness outline. Face-on elevations use frame symbols,
		// avoiding seams from the physical mesh's aperture-subtraction strips.
		if (Math.abs(faceArea) > 0.01) {
			const thickness = entity.sizeMillimetres[1];
			const normalDepth = modelDepth(
				rotateModelPoint([0, 0, 1], entity.rotationDegrees),
				view,
			);
			const faceZ = ((normalDepth <= 0 ? 1 : -1) * thickness) / 2;
			result.lines = geometry.elevation.map((line) => {
				const points = line.map(([x, y, z]) =>
					rotateModelPoint([x, y, z + faceZ], entity.rotationDegrees),
				) as [[number, number, number], [number, number, number]];
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
			result.outlines = [];
		}
	}
	return result;
}
