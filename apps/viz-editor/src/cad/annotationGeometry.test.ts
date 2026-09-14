import { describe, expect, it } from "vitest";
import type { CadAnnotation } from "./annotations";
import {
	annotationLabels,
	annotationRuns,
	annotationsForView,
	formatMeasurement,
	hitAnnotation,
	MEASURE_TICK_MILLIMETRES,
	storedPoint,
	viewPoint,
} from "./annotationGeometry";

function item(
	kind: CadAnnotation["kind"],
	points: [number, number][],
	extra: Partial<CadAnnotation> = {},
): CadAnnotation {
	return {
		id: kind,
		view: "top_down",
		kind,
		points,
		closed: false,
		text: "",
		textHeightMillimetres: 250,
		...extra,
	};
}

describe("drawn items on a CAD plan", () => {
	it("stores a point drawn on a turned plan so the same tile draws it back where it was put", () => {
		for (const turns of [0, 1, 2, 3]) {
			const drawn: [number, number] = [1200, -300];
			const kept = storedPoint("top_down", drawn, turns);
			expect(viewPoint("top_down", kept, turns)).toEqual(drawn);
		}
		// Elevations never turn.
		expect(storedPoint("front_to_back", [5, 6], 1)).toEqual([5, 6]);
		expect(storedPoint("top_down", [1000, 0], 1)).toEqual([0, 1000]);
	});

	it("draws a box from two corners as one closed run, and a closed line back to its start", () => {
		expect(annotationRuns(item("box", [[0, 0], [2000, 1000]]))).toEqual([
			[
				[0, 0],
				[2000, 0],
				[2000, 1000],
				[0, 1000],
				[0, 0],
			],
		]);
		const triangle = item(
			"polyline",
			[
				[0, 0],
				[1000, 0],
				[0, 1000],
			],
			{ closed: true },
		);
		expect(annotationRuns(triangle)[0]).toHaveLength(4);
		expect(annotationRuns({ ...triangle, closed: false })[0]).toHaveLength(3);
	});

	it("draws a measurement with a tick across each end and labels its distance at the middle", () => {
		const measure = item("measure", [
			[0, 0],
			[3250, 0],
		]);
		const [line, startTick, endTick] = annotationRuns(measure);
		expect(line).toEqual([
			[0, 0],
			[3250, 0],
		]);
		expect(startTick).toEqual([
			[0, MEASURE_TICK_MILLIMETRES],
			[0, -MEASURE_TICK_MILLIMETRES],
		]);
		expect(endTick[0][0]).toBe(3250);
		expect(annotationLabels([measure])).toEqual([
			{
				id: "measure",
				kind: "measure",
				point: [1625, 0],
				text: "3.25 m",
				heightMillimetres: null,
			},
		]);
		expect(formatMeasurement(840.4)).toBe("840 mm");
	});

	it("finds what the pointer is over: a line near its segment and text over its words", () => {
		const items = [
			item("box", [
				[0, 0],
				[1000, 1000],
			]),
			item("text", [[5000, 0]], { text: "FOH", textHeightMillimetres: 200 }),
		];
		expect(hitAnnotation(items, [500, 20], 0, 50)).toBe("box");
		expect(hitAnnotation(items, [500, 500], 0, 50)).toBeNull();
		expect(hitAnnotation(items, [5100, 100], 0, 50)).toBe("text");
		expect(annotationsForView(items, "front_to_back")).toEqual([]);
	});
});
