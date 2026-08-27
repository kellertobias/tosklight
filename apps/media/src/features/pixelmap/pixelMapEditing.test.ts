import { describe, expect, it } from "vitest";
import type { PixelMapView, PixelZoneView } from "../../shared/api/generated/media-wire";
import {
	footprintOf,
	newRegion,
	newRoute,
	newZone,
	nextFreeAddress,
	pixelMapProblems,
} from "./pixelMapEditing";

function zone(overrides: Partial<PixelZoneView> = {}): PixelZoneView {
	const base: PixelZoneView = {
		id: "zone",
		name: "Zone",
		start: { x: 0, y: 0 },
		end: { x: 1, y: 1 },
		columns: 10,
		rows: 1,
		layout: { name: "RGB", components: ["red", "green", "blue"] },
		order: "row-major",
		universe: 1,
		startAddress: 1,
		enabled: true,
		footprint: 30,
	};
	return { ...base, ...overrides };
}

function map(overrides: Partial<PixelMapView> = {}): PixelMapView {
	return {
		mode: "direct",
		zones: [],
		routes: [{ id: "r", name: "Universe 1", protocol: "art-net", universe: 1, destination: null, enabled: true }],
		regions: [],
		...overrides,
	};
}

describe("adding to a pixel map", () => {
	it("puts a new zone in the middle of the canvas so it can be seen at once", () => {
		const added = newZone([]);
		expect(added.start.x).toBeGreaterThan(0);
		expect(added.end.x).toBeLessThanOrEqual(1);
		expect(added.columns).toBeGreaterThan(0);
	});

	it("gives a second zone the first free address rather than landing it on the first", () => {
		const first = zone({ startAddress: 1, footprint: 30 });
		const added = newZone([first]);
		expect(added.startAddress).toBe(31);
	});

	it("finds a gap between two zones when there is one", () => {
		const zones = [
			zone({ id: "a", startAddress: 1, footprint: 30 }),
			zone({ id: "b", startAddress: 200, footprint: 30 }),
		];
		expect(nextFreeAddress(zones, 1, 30)).toBe(31);
	});

	it("counts only the universe being addressed", () => {
		const elsewhere = [zone({ universe: 4, startAddress: 1, footprint: 30 })];
		expect(nextFreeAddress(elsewhere, 1, 30)).toBe(1);
	});

	it("numbers new routes and regions after the ones already there", () => {
		expect(newRoute([]).universe).toBe(1);
		expect(newRoute([newRoute([])]).universe).toBe(2);
		expect(newRegion([]).name).toBe("Screen 1");
	});
});

describe("what an operator is told is wrong", () => {
	it("says nothing about a sound map", () => {
		expect(pixelMapProblems(map({ zones: [zone()] }))).toEqual([]);
	});

	it("names the two zones that collide and the address they collide at", () => {
		const problems = pixelMapProblems(
			map({ zones: [zone({ id: "a", name: "Left" }), zone({ id: "b", name: "Right", startAddress: 20 })] }),
		);
		expect(problems).toHaveLength(1);
		expect(problems[0]).toContain("Left");
		expect(problems[0]).toContain("Right");
		expect(problems[0]).toContain("address 20");
	});

	it("says when a zone runs past the end of its universe", () => {
		const problems = pixelMapProblems(map({ zones: [zone({ startAddress: 500 })] }));
		expect(problems[0]).toContain("runs past the end of universe 1");
	});

	it("says when a zone has no route to travel on", () => {
		const problems = pixelMapProblems(map({ zones: [zone({ universe: 7 })] }));
		expect(problems[0]).toContain("no enabled output route carries");
	});

	it("asks for no route when the map is handed to the desk", () => {
		expect(
			pixelMapProblems(map({ mode: "desk-merge", routes: [], zones: [zone({ universe: 7 })] })),
		).toEqual([]);
	});

	it("says when a zone has no pixels", () => {
		const problems = pixelMapProblems(map({ zones: [zone({ columns: 0 })] }));
		expect(problems.some((problem) => problem.includes("no pixels"))).toBe(true);
	});

	it("says when a region covers none of the canvas", () => {
		const problems = pixelMapProblems(
			map({ regions: [{ ...newRegion([]), end: { x: 0, y: 1 } }] }),
		);
		expect(problems[0]).toContain("covers none of the canvas");
	});

	it("ignores a disabled zone when looking for collisions", () => {
		const problems = pixelMapProblems(
			map({ zones: [zone({ id: "a" }), zone({ id: "b", enabled: false })] }),
		);
		expect(problems).toEqual([]);
	});

	it("counts a zone's slots from its shape and its layout", () => {
		expect(footprintOf(zone({ columns: 12, rows: 2 }))).toBe(72);
	});
});
