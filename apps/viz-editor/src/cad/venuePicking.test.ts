import { expect, it } from "vitest";
import { pickEntity, worldGeometry } from "./planGeometry";
import { entityPlanGeometry } from "./projection";
import type { CadEntity } from "./types";

type Triangle = [number[], number[], number[]];
function rectangle(
	x1: number,
	x2: number,
	z1: number,
	z2: number,
	height: number,
): Triangle[] {
	return [
		[
			[x1, height, z1],
			[x2, height, z1],
			[x2, height, z2],
		],
		[
			[x1, height, z1],
			[x2, height, z2],
			[x1, height, z2],
		],
	];
}
function loop(
	low: number,
	high: number,
): [[number, number], [number, number]][] {
	return [
		[
			[low, low],
			[high, low],
		],
		[
			[high, low],
			[high, high],
		],
		[
			[high, high],
			[low, high],
		],
		[
			[low, high],
			[low, low],
		],
	];
}
function floor(id: string, height: number, hole = true): CadEntity {
	const triangles = hole
		? [
				...rectangle(-2000, -500, -2000, 2000, height),
				...rectangle(500, 2000, -2000, 2000, height),
				...rectangle(-500, 500, -2000, -500, height),
				...rectangle(-500, 500, 500, 2000, height),
			]
		: rectangle(-2000, 2000, -2000, 2000, height);
	return {
		id,
		logicalFixtureId: id,
		name: id,
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "—",
		kind: "venue",
		fixtureType: "venue",
		drawingId: id,
		layerId: "default",
		selectable: true,
		positionMillimetres: [1000, 2000, 0],
		rotationDegrees: [0, 0, 90],
		sizeMillimetres: [4000, 4000, 200],
		outputDirection: [0, 0, 0],
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: {
				kind: "slab",
				purpose: "balcony",
				outline_millimetres: [
					{ x: -2000, y: -2000 },
					{ x: 2000, y: -2000 },
					{ x: 2000, y: 2000 },
					{ x: -2000, y: 2000 },
				],
				elevation_millimetres: height,
				thickness_millimetres: 200,
			},
		},
		venueGeometry: {
			triangles,
			plan: [...loop(-2000, 2000), ...(hole ? loop(-500, 500) : [])],
		},
	};
}
const camera = { pan: [0, 0] as [number, number], zoom: 1 };
const pick = (point: [number, number], entities: CadEntity[]) =>
	pickEntity(point, entities, new Map(), "top_down", 0, camera);
it("retains physical picking faces while the drawn floor remains linework", () => {
	const balcony = floor("balcony", 3000);
	const geometry = entityPlanGeometry(balcony, undefined, "top_down");
	expect(geometry.triangles).toEqual([]);
	expect(geometry.pickTriangles).toHaveLength(8);
	const world = worldGeometry(balcony, geometry, "top_down", 0);
	expect(world.pickTriangles?.[0].points[0][0]).toBeCloseTo(-1000);
	expect(world.pickTriangles?.[0].points[0][1]).toBeCloseTo(0);
});
it("does not select the floor through its opening, even at the placement datum", () => {
	expect(pick([1000, 2000], [floor("balcony", 3000)])).toBeNull();
	expect(pick([2500, 2000], [floor("balcony", 3000)])?.id).toBe("balcony");
});
it("selects stairs in the floor opening using tread faces rather than a polygon made from plan lines", () => {
	const stairs = {
		...floor("stairs", 0),
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: { kind: "stairs", steps: 2 },
		},
		venueGeometry: {
			triangles: [
				...rectangle(-500, 500, -500, 0, 1500),
				...rectangle(-500, 500, 0, 500, 3000),
			],
			plan: [
				...loop(-500, 500),
				[
					[-500, 0],
					[500, 0],
				],
			],
		},
	} as CadEntity;
	expect(pick([1100, 2000], [floor("balcony", 3000), stairs])?.id).toBe(
		"stairs",
	);
});
it("chooses the upper solid balcony before the lower one despite identical floor placement datums", () => {
	expect(
		pick(
			[1000, 2000],
			[floor("lower", 3000, false), floor("upper", 6000, false)],
		)?.id,
	).toBe("upper");
});
