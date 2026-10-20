import { expect, it } from "vitest";
import { type CadFrame, EntityLayerCache } from "./lineRenderer";
import type { CadEntity } from "./types";

function wall(id: string, marker: number, x = 0): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name: "Wall",
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "—",
		kind: "venue",
		fixtureType: "venue",
		drawingId: "shared-box-profile",
		layerId: "default",
		selectable: true,
		positionMillimetres: [x, 0, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [2000, 200, 3000],
		outputDirection: [0, 0, 0],
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: { kind: "wall", openings: [] },
		},
		venueGeometry: {
			triangles: [
				[
					[-1000, 0, 100],
					[1000, 0, 100],
					[1000, 3000, 100],
				],
				[
					[-1000, 0, 100],
					[1000, 3000, 100],
					[-1000, 3000, 100],
				],
			],
			plan: [],
			elevation: [
				[
					[marker, 1000, 0],
					[marker, 2000, 0],
				],
			],
		},
	};
}
function frame(
	entities: CadEntity[],
	view: CadFrame["view"] = "front_to_back",
): CadFrame {
	return {
		entities,
		drawings: new Map(),
		selected: new Set(),
		view,
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.1 },
		preview: null,
		editEnabled: true,
		guide: null,
		selectionBox: null,
	};
}
it("paints distinct equal-size walls with the same carrier profile and name using their own window geometry", () => {
	const cache = new EntityLayerCache();
	const layer = cache.layers(
		frame([wall("a", 100), wall("b", 900, 5000)]),
	).still;
	expect(layer.depthLines.length).toBe(28);
	expect(layer.depthLines[0]).toBeCloseTo(100);
	expect(layer.depthLines[14]).toBeCloseTo(5900);
	// Frames are on the nearest wall face (-100 mm) with the painter's 1 mm line bias.
	expect(layer.depthLines[2]).toBeCloseTo(-101 / 100000);
});
it("repaints an edited wall even when entity, drawing, dimensions and rotation stay the same", () => {
	const cache = new EntityLayerCache();
	expect(cache.layers(frame([wall("a", 100)])).still.depthLines[0]).toBeCloseTo(
		100,
	);
	expect(cache.layers(frame([wall("a", 800)])).still.depthLines[0]).toBeCloseTo(
		800,
	);
});
it("rebuilds a floor's contextual stair-opening plan when native geometry changes", () => {
	const source = wall("floor", 0);
	const floor = {
		...source,
		scenery: {
			...source.scenery,
			architecture: {
				kind: "slab",
				purpose: "floor",
				outline_millimetres: [
					{ x: -1000, y: -1000 },
					{ x: 1000, y: -1000 },
					{ x: 1000, y: 1000 },
					{ x: -1000, y: 1000 },
				],
				elevation_millimetres: 0,
				thickness_millimetres: 200,
			},
		},
		venueGeometry: {
			triangles: [],
			plan: [
				[
					[-1000, -1000],
					[1000, -1000],
				],
				[
					[1000, -1000],
					[1000, 1000],
				],
				[
					[1000, 1000],
					[-1000, 1000],
				],
				[
					[-1000, 1000],
					[-1000, -1000],
				],
			],
		},
	} as CadEntity;
	const cache = new EntityLayerCache();
	const before = cache.layers(frame([floor], "top_down")).still;
	if (!floor.venueGeometry) throw new Error("Missing floor geometry");
	const cut = {
		...floor,
		venueGeometry: {
			triangles: [],
			plan: [
				...floor.venueGeometry.plan,
				[
					[-500, -500],
					[500, -500],
				],
				[
					[500, -500],
					[500, 500],
				],
				[
					[500, 500],
					[-500, 500],
				],
				[
					[-500, 500],
					[-500, -500],
				],
			],
		},
	} as CadEntity;
	const after = cache.layers(frame([cut], "top_down")).still;
	expect(before.depthLines.length).toBe(4 * 14);
	expect(after.depthLines.length).toBe(8 * 14);
});
