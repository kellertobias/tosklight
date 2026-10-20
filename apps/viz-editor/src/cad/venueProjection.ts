/** The shared venue solids and plan symbols, including real openings and door swing arcs. */
import {
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
	return result;
}
