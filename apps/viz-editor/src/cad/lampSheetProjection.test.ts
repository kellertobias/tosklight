import front from "../../../../assets/models/2d/lamps/profile-spot/front.svg?raw";
import side from "../../../../assets/models/2d/lamps/profile-spot/side.svg?raw";
import top from "../../../../assets/models/2d/lamps/profile-spot/top.svg?raw";
import { describe, expect, it } from "vitest";
import { modelDrawingGeometry } from "./modelDrawing";
import { worldGeometry } from "./planGeometry";
import type { PlanGeometry } from "./projection";
import type { CadDrawing, CadEntity, CadViewDirection } from "./types";

const model: CadDrawing = {
	id: "elp-profile",
	projections: [],
	liveMeshes: [],
	modelDrawing: {
		model: "profile-spot",
		scale: 1,
		views: (["top", "front", "side"] as const).map((view) => ({
			view,
			svg: { top, front, side }[view],
		})),
	},
};
const lamp = {
	id: "elp-5031",
	kind: "fixture",
	fixtureType: "led profile",
	positionMillimetres: [0, 0, 5000],
	rotationDegrees: [65.897765, -13.507398, 180],
} as CadEntity;

function area(geometry: PlanGeometry) {
	return geometry.triangles.reduce((total, { points: [a, b, c] }) =>
		total + Math.abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2, 0);
}

// Real shipped artwork: a valid profile model must not collapse when the show aims the lamp.
describe("the ELP profiler's canonical drawing", () => {
	for (const view of ["top_down", "front_to_back", "left_to_right"] satisfies CadViewDirection[]) {
		it(`keeps its silhouette and linework readable in ${view}`, () => {
			const canonical = modelDrawingGeometry(model, view)!;
			expect(canonical.lines.length).toBeGreaterThan(10);
			expect(area(canonical)).toBeGreaterThan(10000);
			for (const rotationDegrees of [[65.897765, -13.507398, 180], [90, 90, 180], [17, 29, 41]] as [number, number, number][]) {
				const placed = worldGeometry({ ...lamp, rotationDegrees }, canonical, view, 0);
				expect(placed.lines.length).toBe(canonical.lines.length);
				expect(area(placed)).toBeCloseTo(area(canonical), 5);
			}
		});
	}
});
