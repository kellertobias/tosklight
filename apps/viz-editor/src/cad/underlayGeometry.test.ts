import { describe, expect, it } from "vitest";
import type { CadPrintPage } from "./types";
import {
	pageShowsUnderlay,
	placedExtents,
	placedPolylines,
	underlayKey,
	underlaysForPage,
	underlaysForView,
	withUnderlayShown,
} from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";

function drawing(overrides: Partial<CadUnderlay> = {}): CadUnderlay {
	return {
		id: "plan",
		name: "Ground plan.dxf",
		sourceFormat: "dxf",
		view: "top_down",
		originMillimetres: [0, 0],
		scale: 1,
		rotationDegrees: 0,
		visible: true,
		units: "millimetres",
		geometry: {
			polylines: [
				{
					points: [
						[0, 0],
						[1000, 0],
						[1000, 500],
					],
					closed: false,
					layer: "A-WALL",
				},
			],
			extentsMillimetres: [0, 0, 1000, 500],
			units: "millimetres",
		},
		...overrides,
	};
}

function page(overrides: Partial<CadPrintPage> = {}): CadPrintPage {
	return {
		kind: "plan",
		id: "page-1",
		tileId: "tile-1",
		name: "Page 1",
		view: "top_down",
		rotationQuarterTurns: 0,
		centreMillimetres: [0, 0],
		widthMillimetres: 10_000,
		included: true,
		orientation: "landscape",
		showFixtureIds: false,
		showDmxAddresses: false,
		...overrides,
	};
}

describe("placing a drawing", () => {
	it("moves and scales it into the show", () => {
		const [run] = placedPolylines(
			drawing({ originMillimetres: [2000, 1000], scale: 2 }),
		);
		expect(run[0]).toEqual([2000, 1000]);
		expect(run[1]).toEqual([4000, 1000]);
		expect(run[2]).toEqual([4000, 2000]);
	});

	it("turns it about its own origin", () => {
		const [run] = placedPolylines(drawing({ rotationDegrees: 90 }));
		expect(run[1][0]).toBeCloseTo(0);
		expect(run[1][1]).toBeCloseTo(1000);
	});

	it("turns with a rotated top-down tile, as the rig does", () => {
		const [straight] = placedPolylines(drawing(), 0);
		const [turned] = placedPolylines(drawing(), 1);
		expect(straight[1]).toEqual([1000, 0]);
		// A quarter turn clockwise puts what ran to the right at the bottom of the plan.
		expect(turned[1][0]).toBeCloseTo(0);
		expect(turned[1][1]).toBeCloseTo(-1000);
	});

	it("leaves an elevation alone, which cannot be turned", () => {
		const [run] = placedPolylines(drawing({ view: "front_to_back" }), 1);
		expect(run[1]).toEqual([1000, 0]);
	});

	it("closes a closed run so it can be drawn as one polyline", () => {
		const closed = drawing();
		closed.geometry.polylines[0].closed = true;
		const [run] = placedPolylines(closed);
		expect(run).toHaveLength(4);
		expect(run[3]).toEqual(run[0]);
	});

	it("measures the box it covers after placement", () => {
		expect(placedExtents(drawing({ originMillimetres: [100, 0] }))).toEqual([
			100, 0, 1100, 500,
		]);
	});

	it("keys a cache by everything that would draw differently", () => {
		const first = underlayKey(drawing(), 0);
		expect(underlayKey(drawing(), 0)).toBe(first);
		expect(underlayKey(drawing({ scale: 2 }), 0)).not.toBe(first);
		expect(underlayKey(drawing(), 1)).not.toBe(first);
	});
});

describe("which drawings a view and a page show", () => {
	const plan = drawing({ id: "plan" });
	const section = drawing({ id: "section", view: "front_to_back" });
	const hidden = drawing({ id: "hidden", visible: false });
	const all = [plan, section, hidden];

	it("shows only the visible drawings of that axis", () => {
		expect(underlaysForView(all, "top_down").map((one) => one.id)).toEqual([
			"plan",
		]);
		expect(underlaysForView(all, "front_to_back").map((one) => one.id)).toEqual([
			"section",
		]);
	});

	it("prints every drawing on the page's axis by default", () => {
		expect(underlaysForPage(all, page()).map((one) => one.id)).toEqual(["plan"]);
		expect(pageShowsUnderlay(page(), "plan")).toBe(true);
	});

	it("prints nothing a page has switched off", () => {
		const off = page({ hiddenUnderlayIds: ["plan"] });
		expect(underlaysForPage(all, off)).toEqual([]);
		expect(pageShowsUnderlay(off, "plan")).toBe(false);
	});

	it("switches one drawing on and off without touching the others", () => {
		const off = withUnderlayShown(page(), "plan", false);
		expect(off).toEqual(["plan"]);
		expect(withUnderlayShown({ hiddenUnderlayIds: off }, "plan", true)).toEqual(
			[],
		);
	});
});
