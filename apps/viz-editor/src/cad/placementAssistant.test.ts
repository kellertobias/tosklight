import { describe, expect, it } from "vitest";
import { assistantPositions } from "./placementAssistant";

describe("Placement Assistant", () => {
	it("spaces a line evenly from its start to its end", () => {
		expect(
			assistantPositions(
				{ shape: "line", start: { x: -2, y: 0, z: 6 }, end: { x: 2, y: 1, z: 6 } },
				3,
			),
		).toEqual([
			{ x: -2000, y: 0, z: 6000 },
			{ x: 0, y: 500, z: 6000 },
			{ x: 2000, y: 1000, z: 6000 },
		]);
	});

	it("fills a grid row by row", () => {
		expect(
			assistantPositions(
				{ shape: "grid", start: { x: 0, y: 0, z: 5 }, columns: 2, spacingX: 1.5, spacingY: 2 },
				3,
			),
		).toEqual([
			{ x: 0, y: 0, z: 5000 },
			{ x: 1500, y: 0, z: 5000 },
			{ x: 0, y: 2000, z: 5000 },
		]);
	});

	it("divides a whole circle by the count and an arc from end to end", () => {
		const whole = assistantPositions(
			{ shape: "circle", centre: { x: 0, y: 0, z: 4 }, radius: 2, startAngle: 0, arc: 360 },
			4,
		);
		expect(whole.map(({ x, y }) => [x, y])).toEqual([
			[2000, 0],
			[0, 2000],
			[-2000, 0],
			[0, -2000],
		]);
		const half = assistantPositions(
			{ shape: "circle", centre: { x: 0, y: 0, z: 4 }, radius: 1, startAngle: 0, arc: 180 },
			3,
		);
		expect(half.map(({ x, y }) => [x, y])).toEqual([
			[1000, 0],
			[0, 1000],
			[-1000, 0],
		]);
	});
});
