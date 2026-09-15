import { describe, expect, it } from "vitest";
import { gridGeometry } from "./cadGrid";

const camera = { zoom: 0.1, pan: [0, 0] as [number, number] };

describe("the CAD grid", () => {
	it("draws a line at every step across the view, through the plan origin", () => {
		// 200 × 100 px at 0.1 px/mm shows ±1 m by ±0.5 m; a 500 mm step lands 5 columns and 3 rows.
		const { lines, crosses } = gridGeometry({
			width: 200,
			height: 100,
			camera,
			stepMillimetres: 500,
			subdivisions: 0,
		});
		const vertical = lines.filter((line) => line.x1 === line.x2).map((line) => line.x1);
		const horizontal = lines.filter((line) => line.y1 === line.y2).map((line) => line.y1);
		expect(vertical).toEqual([0, 50, 100, 150, 200]);
		expect(horizontal).toEqual([100, 50, 0]);
		expect(crosses).toEqual([]);
	});

	it("follows the camera's pan", () => {
		const { lines } = gridGeometry({
			width: 200,
			height: 100,
			camera: { zoom: 0.1, pan: [250, 0] },
			stepMillimetres: 500,
			subdivisions: 0,
		});
		// Panned 250 mm, the plan's origin sits 25 px right of centre and the lines move with it.
		expect(lines.filter((line) => line.x1 === line.x2).map((line) => line.x1)).toEqual([
			25, 75, 125, 175,
		]);
	});

	it("puts plus signs at the quarter steps, never on a grid line", () => {
		const { crosses } = gridGeometry({
			width: 200,
			height: 200,
			camera: { zoom: 0.1, pan: [0, 0] },
			stepMillimetres: 1000,
			subdivisions: 4,
		});
		// Quarter steps are 25 px apart; the lines at 0, 100 and 200 px carry none.
		const columns = [...new Set(crosses.map((cross) => cross.x))].sort((a, b) => a - b);
		expect(columns).toEqual([25, 50, 75, 125, 150, 175]);
		expect(crosses).toHaveLength(36);
	});

	it("leaves out a grid too dense to read", () => {
		expect(
			gridGeometry({ width: 200, height: 100, camera, stepMillimetres: 20, subdivisions: 4 }),
		).toEqual({ lines: [], crosses: [] });
		expect(
			gridGeometry({ width: 200, height: 100, camera, stepMillimetres: 100, subdivisions: 4 })
				.crosses,
		).toEqual([]);
	});
});
