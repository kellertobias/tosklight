import { describe, expect, it } from "vitest";
import { fittedCamera } from "./CadApp";
import { entityPlanGeometry, isPositionPoint } from "./projection";
import { worldGeometry } from "./planGeometry";
import {
	CAD_VIEW_LABELS, directionIndicator, projectPoint,
	type CadDrawing, type CadEntity, type CadViewDirection,
} from "./types";

const rig: CadEntity[] = [{
	id: "venue", logicalFixtureId: "venue", name: "Wide rotated venue", fixtureNumber: 1,
	fixtureDisplayId: "1", dmxAddress: "", kind: "venue", fixtureType: "venue",
	drawingId: "venue", layerId: "default", selectable: true,
	positionMillimetres: [9000, -4000, 6000], rotationDegrees: [15, 25, 35],
	sizeMillimetres: [30000, 12000, 18000],
	scenery: { kind: "box", chords: 0, pattern: "standard" },
	outputDirection: [0, 1, 0],
}];
rig.push({
	...rig[0], id: "fixture", logicalFixtureId: "fixture", kind: "profile",
	fixtureType: "moving_head_profile", scenery: undefined,
	positionMillimetres: [24000, 12000, 6000], rotationDegrees: [0, 0, 0],
	sizeMillimetres: [400, 500, 700], emitterOffsetMillimetres: [300, 200, 100],
});
const drawings = new Map<string, CadDrawing>();

function assertFits(view: CadViewDirection, turn: number, width: number, height: number) {
	const camera = fittedCamera(rig, view, turn, drawings, width, height);
	for (const entity of rig) {
		const geometry = worldGeometry(
			entity, entityPlanGeometry(entity, drawings.get(entity.drawingId), view), view, turn,
		);
		const points = [
			...geometry.triangles.flatMap(triangle => triangle.points),
			...geometry.outlines.flat(), ...geometry.lines.flatMap(line => line.points),
		];
		expect(points.length).toBeGreaterThan(0);
		if (entity.kind !== "venue" && !isPositionPoint(entity)) {
			points.push(...directionIndicator(
				entity, projectPoint(entity.positionMillimetres, view, turn), view, turn,
			));
		}
		for (const point of points) {
			const x = width / 2 + (point[0] + camera.pan[0]) * camera.zoom;
			const y = height / 2 - (point[1] + camera.pan[1]) * camera.zoom;
			expect(x).toBeGreaterThanOrEqual(32 - 1e-6);
			expect(x).toBeLessThanOrEqual(width - 32 + 1e-6);
			expect(y).toBeGreaterThanOrEqual(32 - 1e-6);
			expect(y).toBeLessThanOrEqual(height - 32 + 1e-6);
		}
	}
	return camera;
}

describe("CAD Fit visible geometry", () => {
	it("fits actual rotated venue extents into unequal split sizes in every direction", () => {
		for (const view of Object.keys(CAD_VIEW_LABELS) as CadViewDirection[]) {
			for (const turn of view === "top_down" ? [0, 1, 2, 3] : [0]) {
				assertFits(view, turn, 410, 760);
				assertFits(view, turn, 900, 330);
			}
		}
	});

	it("recomputes the CSS viewport bounds after resizing and preserves entity data", () => {
		const before = JSON.stringify(rig);
		const large = assertFits("top_down", 1, 1000, 800);
		const smaller = assertFits("top_down", 1, 380, 320);
		expect(smaller.zoom).toBeLessThan(large.zoom);
		expect(JSON.stringify(rig)).toBe(before);
	});
});
