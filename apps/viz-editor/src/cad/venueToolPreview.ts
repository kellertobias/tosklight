import type { CadDrawTool } from "./cadTools";
import type { CadViewportContext } from "./useCadViewportInteraction";
import { projectPoint } from "./types";
import type { PlanPoint } from "./projection";
import { newArchitecture } from "./architectureDraft";
import { openingPreview } from "./venueDrawingGeometry";
import type { VenueToolPreview } from "./useCadVenueTool";
type World = [number, number, number];
export function venueToolPreview(
	context: CadViewportContext,
	tool: CadDrawTool,
	start: World | null,
	cursor: PlanPoint | null,
	world: (point: PlanPoint) => World,
	threshold: number,
): VenueToolPreview | null {
	let preview: VenueToolPreview | null = null;
	if (cursor) {
		if (tool === "window" || tool === "door") {
			const fit = openingPreview(
				context.entities,
				cursor,
				tool,
				context.view,
				context.rotationQuarterTurns,
				threshold,
			);
			if (fit)
				preview = {
					kind: tool,
					points: fit.points,
					valid: fit.valid,
					wallId: fit.wall.id,
				};
		} else if (context.view === "top_down") {
			const end = world(cursor);
			if (tool === "wall" && start) {
				const dx = end[0] - start[0],
					dy = end[1] - start[1],
					length = Math.hypot(dx, dy) || 1,
					nx = (-dy / length) * 100,
					ny = (dx / length) * 100;
				preview = {
					kind: "wall",
					valid: true,
					points: [
						[start[0] + nx, start[1] + ny, start[2]],
						[end[0] + nx, end[1] + ny, end[2]],
						[end[0] - nx, end[1] - ny, end[2]],
						[start[0] - nx, start[1] - ny, start[2]],
					].map((p) =>
						projectPoint(
							p as World,
							context.view,
							context.rotationQuarterTurns,
						),
					),
				};
			} else if (tool === "pillar" || tool === "stairs") {
				const size = newArchitecture(tool).size;
				preview = {
					kind: tool,
					valid: true,
					points: [
						[-1, -1],
						[1, -1],
						[1, 1],
						[-1, 1],
					].map(([x, y]) =>
						projectPoint(
							[end[0] + (x * size.x) / 2, end[1] + (y * size.z) / 2, end[2]],
							context.view,
							context.rotationQuarterTurns,
						),
					),
				};
			}
		}
	}
	return preview;
}
