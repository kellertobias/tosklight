import { describe, expect, it } from "vitest";
import { entityDepthRange } from "./cutPlanes";
import { entityBounds } from "./marqueeSelection";
import { type FreeAxes, snapMove, snapPlanPoint } from "./snapping";
import { trussParts } from "./trussPlan";
import type { CadEntity } from "./types";
import { trussConnectors } from "./venueShapes";

type V3 = [number, number, number];

function base(id: string, extra: Partial<CadEntity>): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name: id,
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "Visual only",
		kind: "venue",
		fixtureType: "venue",
		drawingId: id,
		layerId: "default",
		selectable: true,
		positionMillimetres: [0, 0, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [1000, 1000, 1000],
		outputDirection: [0, 1, 0],
		...extra,
	};
}

const truss = (id: string, position: V3, size: V3 = [4000, 290, 290], chords = 4) =>
	base(id, {
		positionMillimetres: position,
		sizeMillimetres: size,
		scenery: { kind: "truss", chords, pattern: "standard" },
	});
const riser = (id: string, position: V3, size: V3) =>
	base(id, {
		positionMillimetres: position,
		sizeMillimetres: size,
		scenery: { kind: "riser", chords: 0, pattern: "standard" },
	});
const curtain = (id: string, position: V3, size: V3 = [3000, 60, 6000]) =>
	base(id, {
		positionMillimetres: position,
		sizeMillimetres: size,
		scenery: { kind: "curtain", chords: 0, pattern: "standard" },
	});

const PLAN: FreeAxes = [true, true, false];
const FRONT: FreeAxes = [true, false, true];

function expectVector(actual: readonly number[], expected: readonly number[]) {
	expect(actual).toHaveLength(expected.length);
	actual.forEach((value, index) => expect(value).toBeCloseTo(expected[index], 3));
}

describe("CAD snapping", () => {
	it("joins two trusses where their connectors meet", () => {
		const still = truss("a", [0, 0, 5000]);
		const moving = truss("b", [4100, 0, 5000]);
		const snapped = snapMove([still, moving], ["b"], [-50, 30, 0], PLAN, 150);
		expectVector(snapped.delta, [-100, 0, 0]);
		expectVector(snapped.targets[0], [2000, 0, 5000]);
		// Beyond the snap distance the drag is left alone.
		const far = snapMove([still, moving], ["b"], [-300, 0, 0], PLAN, 150);
		expect(far).toEqual({ delta: [-300, 0, 0], targets: [] });
	});

	it("finds a corner piece's connectors at the end of its 500 mm arms", () => {
		const corner = base("c", { fixtureProfile: "Venue Four-Point Truss Corner 2-Way" });
		const [first, second] = trussConnectors(corner);
		expectVector(first, [-500, 0, 0]);
		expectVector(second, [0, 500, 0]);
	});

	it("puts two stage elements corner to corner", () => {
		const still = riser("a", [0, 0, 0], [2000, 1000, 400]);
		const moving = riser("b", [2050, 20, 0], [2000, 1000, 400]);
		expectVector(snapMove([still, moving], ["b"], [10, 0, 0], PLAN, 150).delta, [-50, -20, 0]);
	});

	it("stands a stage element's feet on the top of another", () => {
		const deck = base("a", {
			fixtureProfile: "Venue Stage Deck 2 × 1 m, Legs 0.4 m",
			sizeMillimetres: [2000, 1000, 440],
		});
		const moving = riser("b", [300, 0, 500], [1000, 1000, 400]);
		const snapped = snapMove([deck, moving], ["b"], [0, 0, -40], FRONT, 150);
		expectVector(snapped.delta, [0, 0, -60]);
		expect(snapped.targets[0][2]).toBe(440);
		// Seen from above, height is not the drag's to change.
		expectVector(snapMove([deck, moving], ["b"], [0, 5, 0], PLAN, 150).delta, [0, 5, 0]);
	});

	it("hangs a curtain's rail under a pipe and lines its ends up with the next curtain", () => {
		const pipe = truss("p", [0, 0, 6000], [4000, 50, 50], 1);
		const drape = curtain("c", [100, 80, 2950]);
		expectVector(snapMove([pipe, drape], ["c"], [0, 0, 5], FRONT, 150).delta, [0, 0, 25]);
		const neighbour = curtain("n", [0, 0, 3000]);
		const next = curtain("m", [3040, 0, 3000]);
		expectVector(snapMove([neighbour, next], ["m"], [0, 10, 0], PLAN, 150).delta, [-40, 0, 0]);
	});

	it("clamps a lamp onto the nearest truss pipe, but not onto one far above it", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		const half = trussParts(290, 4).spacing / 2;
		const lamp = base("l", { kind: "profile", positionMillimetres: [500, 100, 4800] });
		expectVector(snapMove([box, lamp], ["l"], [0, 5, 0], PLAN, 150).delta, [0, half - 100, 0]);
		const floor = { ...lamp, positionMillimetres: [500, 100, 0] as V3 };
		expectVector(snapMove([box, floor], ["l"], [0, 5, 0], PLAN, 150).delta, [0, 5, 0]);
	});

	it("snaps a measurement's point onto a truss connector", () => {
		const rig = [truss("a", [0, 0, 5000])];
		expectVector(snapPlanPoint(rig, [1990, 30], "top_down", 0, 150) ?? [], [2000, 0]);
		expect(snapPlanPoint(rig, [1500, 300], "top_down", 0, 150)).toBeNull();
	});
});

describe("stage elements stand on their position", () => {
	it("reaches from the floor to its height in the marquee and the cut depth", () => {
		const deck = riser("a", [0, 0, 1000], [2000, 1000, 400]);
		expect(entityBounds(deck, "front_to_back")).toEqual({
			minimum: [-1000, 1000],
			maximum: [1000, 1400],
		});
		const range = entityDepthRange(deck, "top_down");
		expect(range.near).toBeCloseTo(-1400);
		expect(range.far).toBeCloseTo(-1000);
		// Anything else is still centred on its position.
		const box = base("b", { positionMillimetres: [0, 0, 1000], sizeMillimetres: [2000, 1000, 400] });
		expect(entityBounds(box, "front_to_back").minimum[1]).toBe(800);
	});
});
