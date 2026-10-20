import { expect, it } from "vitest";
import { type CadFrame, EntityLayerCache } from "./lineRenderer";
import { pickEntity } from "./planGeometry";
import { entityPlanGeometry } from "./projection";
import type { CadEntity, CadViewDirection } from "./types";
import { outlineVenueGeometry, outlineVenueIds } from "./venueViewVisibility";

function wall(
	id: string,
	position: [number, number, number],
	yaw = 0,
): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name: id,
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "—",
		kind: "venue",
		fixtureType: "venue",
		drawingId: "wall",
		layerId: "default",
		selectable: true,
		positionMillimetres: position,
		rotationDegrees: [0, 0, yaw],
		sizeMillimetres: [4000, 200, 3000],
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
					[-2000, 0, 100],
					[2000, 0, 100],
					[2000, 3000, 100],
				],
				[
					[-2000, 0, 100],
					[2000, 3000, 100],
					[-2000, 3000, 100],
				],
			],
			plan: [
				[
					[-2000, -100],
					[2000, -100],
				],
				[
					[2000, -100],
					[2000, 100],
				],
				[
					[2000, 100],
					[-2000, 100],
				],
				[
					[-2000, 100],
					[-2000, -100],
				],
			],
			elevation: [
				[
					[-2000, 0, 0],
					[2000, 0, 0],
				],
				[
					[2000, 0, 0],
					[2000, 3000, 0],
				],
				[
					[2000, 3000, 0],
					[-2000, 3000, 0],
				],
				[
					[-2000, 3000, 0],
					[-2000, 0, 0],
				],
				[
					[-300, 700, 0],
					[-300, 1700, 0],
				],
			],
		},
	};
}
function fixture(): CadEntity {
	return {
		...wall("interior", [0, 0, 1500]),
		kind: "fixture",
		fixtureType: "dimmer",
		drawingId: "lamp",
		sizeMillimetres: [400, 400, 400],
		scenery: undefined,
		venueGeometry: undefined,
	};
}
function frame(entities: CadEntity[], view: CadViewDirection): CadFrame {
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
it("outlines the foreground wall and retains the far wall in either front elevation", () => {
	const entities = [
		wall("front", [0, -2000, 0]),
		wall("back", [0, 2000, 0]),
		fixture(),
	];
	expect([...outlineVenueIds(entities, "front_to_back")]).toEqual(["front"]);
	expect([...outlineVenueIds(entities, "back_to_front")]).toEqual(["back"]);
	expect([...outlineVenueIds(entities, "top_down")]).toEqual([]);
});
it("switches the outlined wall with left/right view while retaining edge-on walls", () => {
	const entities = [
		wall("left", [-2000, 0, 0], 90),
		wall("right", [2000, 0, 0], 90),
		wall("edge-on", [0, 2000, 0]),
		fixture(),
	];
	expect([...outlineVenueIds(entities, "left_to_right")]).toEqual(["left"]);
	expect([...outlineVenueIds(entities, "right_to_left")]).toEqual(["right"]);
});
it("keeps isolated walls and walls that do not cover any venue content solid", () => {
	const front = wall("front", [0, -2000, 0]);
	expect([...outlineVenueIds([front], "front_to_back")]).toEqual([]);
	expect([
		...outlineVenueIds(
			[front, { ...fixture(), positionMillimetres: [10000, 0, 1500] }],
			"front_to_back",
		),
	]).toEqual([]);
});
it("keeps opening symbols and the physical source geometry while drawing no foreground mask", () => {
	const front = wall("front", [0, -2000, 0]);
	const physical = entityPlanGeometry(front, undefined, "front_to_back");
	const drawn = outlineVenueGeometry(physical);
	expect(physical.triangles.length).toBeGreaterThan(0);
	expect(drawn.triangles).toEqual([]);
	expect(drawn.pickTriangles).toEqual([]);
	expect(drawn.lines).toEqual(expect.arrayContaining(physical.lines));
	expect(front.venueGeometry?.triangles).toHaveLength(2);
});
it("paints one far-wall mask and lets the interior fixture and foreground window frames be selected", () => {
	const entities = [
		wall("front", [0, -2000, 0]),
		wall("back", [0, 2000, 0]),
		fixture(),
	];
	const cache = new EntityLayerCache();
	const base = cache.layers(frame(entities, "front_to_back")).still;
	const withoutNear = cache.layers(
		frame(entities.slice(1), "front_to_back"),
	).still;
	expect(base.fill.length).toBe(withoutNear.fill.length);
	expect(base.depthLines.length).toBeGreaterThan(withoutNear.depthLines.length);
	expect(
		pickEntity([0, 1500], entities, new Map(), "front_to_back", 0, {
			pan: [0, 0],
			zoom: 0.1,
		})?.id,
	).toBe("interior");
	expect(
		pickEntity([-300, 1000], entities, new Map(), "front_to_back", 0, {
			pan: [0, 0],
			zoom: 0.1,
		})?.id,
	).toBe("front");
});
it("keeps the scene/view visibility decision cached across repeated frames", () => {
	const entities = [wall("front", [0, -2000, 0]), fixture()];
	const first = outlineVenueIds(entities, "front_to_back");
	for (let i = 0; i < 120; i++)
		expect(outlineVenueIds(entities, "front_to_back")).toBe(first);
	const moved = entities.map((entity) =>
		entity.id === "front"
			? {
					...entity,
					positionMillimetres: [0, 2000, 0] as [number, number, number],
				}
			: entity,
	);
	expect([...outlineVenueIds(moved, "front_to_back")]).toEqual([]);
});
