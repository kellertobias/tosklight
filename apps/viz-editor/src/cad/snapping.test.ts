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
/**
 * A metre-square lamp hanging from the clamp its profile declares: a quarter of its height as a
 * band off the top, with the pipe line at the very top. That is the clamp the shipped packages
 * carry, and the one the plan used to guess before they declared it.
 */
const lamp = (id: string, position: V3) =>
	base(id, {
		kind: "profile",
		positionMillimetres: position,
		mounting: {
			hardware: "clamp",
			centre: [0, 0, 375],
			halfExtent: [500, 500, 125],
			pipe: [0, 0, 500],
		},
	});
const railing = (id: string, position: V3, size: V3 = [2000, 40, 1000]) =>
	base(id, {
		positionMillimetres: position,
		sizeMillimetres: size,
		scenery: { kind: "railing", chords: 0, pattern: "standard" },
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
		expect(far).toEqual({ delta: [-300, 0, 0], targets: [], guides: [] });
	});

	it("finds a corner piece's connectors at the end of its arms, whatever size the block is", () => {
		const block = (part: string, size: V3) =>
			base("c", { fixtureProfile: `Venue Four-Point Truss ${part}`, sizeMillimetres: size });
		// A 500 mm corner: the arm alone on its axis reaches the far face, leaving the 290 mm
		// section of the arm that turns away from it behind.
		const [left, back] = trussConnectors(block("Corner 2-Way", [500, 500, 290]));
		expectVector(left, [-355, 0, 0]);
		expectVector(back, [0, 355, 0]);
		// A cross is 500 mm across each axis it has an arm each way on, so those arms take half.
		expectVector(trussConnectors(block("Cross 4-Way", [500, 500, 290]))[0], [-250, 0, 0]);
		expectVector(trussConnectors(block("Node 6-Way", [500, 500, 500]))[4], [0, 0, 250]);
		// The larger blocks older shows were built with still join where their couplers are.
		expectVector(trussConnectors(block("Corner 2-Way", [679, 679, 290]))[0], [-534, 0, 0]);
		expectVector(trussConnectors(block("Cross 4-Way", [1068, 1068, 290]))[0], [-534, 0, 0]);
	});

	it("joins a corner block onto the end of a straight truss", () => {
		// Corners are not mountable, but they must still couple to the run they turn.
		const run = truss("a", [0, 0, 5000]);
		const corner = base("c", {
			fixtureProfile: "Venue Four-Point Truss Corner 2-Way",
			sizeMillimetres: [500, 500, 290],
			positionMillimetres: [2375, 20, 5000],
		});
		// Its left arm reaches 355 mm back, so it couples at 2355 against the run's end at 2000.
		expectVector(snapMove([run, corner], ["c"], [0, 0, 0], PLAN, 150).delta, [-20, -20, 0]);
	});

	it("puts two stage elements corner to corner", () => {
		const still = riser("a", [0, 0, 0], [2000, 1000, 400]);
		const moving = riser("b", [2050, 20, 0], [2000, 1000, 400]);
		expectVector(snapMove([still, moving], ["b"], [10, 0, 0], PLAN, 150).delta, [-50, -20, 0]);
	});

	it("couples a truss only to a connector of the same system", () => {
		const four = truss("a", [0, 0, 5000]);
		const three = truss("b", [4100, 0, 5000], [4000, 290, 290], 3);
		expectVector(snapMove([four, three], ["b"], [-50, 0, 0], PLAN, 150).delta, [-50, 0, 0]);
		// A corner block says its system in its name: a three-point corner takes only a three-point run.
		const corner = base("c", {
			fixtureProfile: "Venue Three-Point Truss Corner 2-Way",
			sizeMillimetres: [500, 500, 290],
			positionMillimetres: [2375, 20, 5000],
		});
		expectVector(snapMove([four, corner], ["c"], [0, 0, 0], PLAN, 150).delta, [0, 0, 0]);
		const run = truss("r", [0, 0, 5000], [4000, 290, 290], 3);
		expectVector(snapMove([run, corner], ["c"], [0, 0, 0], PLAN, 150).delta, [-20, -20, 0]);
	});

	it("butts a stage element against a neighbour's side wherever along it the drag lets go", () => {
		const wide = riser("a", [0, 0, 0], [2000, 1000, 400]);
		// A metre-square deck 40 mm clear of the wide deck's back side, 300 mm along it: no corner
		// is anywhere near, but the sides meet.
		const moving = riser("b", [300, 1040, 0], [1000, 1000, 600]);
		const snapped = snapMove([wide, moving], ["b"], [5, 0, 0], PLAN, 150);
		expectVector(snapped.delta, [5, -40, 0]);
		// The guide runs along the joined side, across both decks, at the higher top.
		expect(snapped.guides).toHaveLength(1);
		expectVector(snapped.guides[0][0], [-1000, 500, 600]);
		expectVector(snapped.guides[0][1], [1000, 500, 600]);
		expect(snapped.targets).toHaveLength(1);
		// Beyond the snap distance it stays where it was dragged.
		const far = snapMove([wide, { ...moving, positionMillimetres: [300, 1400, 0] }], ["b"], [5, 0, 0], PLAN, 150);
		expect(far).toEqual({ delta: [5, 0, 0], targets: [], guides: [] });
	});

	it("lines a stage element's side up flush with a neighbour's and butts it on the other axis", () => {
		const still = riser("a", [0, 0, 0], [2000, 1000, 400]);
		// Beside the deck on the right, 60 mm clear and 100 mm proud of its front.
		const moving = riser("b", [1560, -100, 0], [1000, 1000, 400]);
		const snapped = snapMove([still, moving], ["b"], [0, 0, 0], PLAN, 150);
		expectVector(snapped.delta, [-60, 100, 0]);
		expect(snapped.guides).toHaveLength(2);
		// The two sides, drawn where the deck landed: the joined side at x = 1000 and the flush
		// front at y = -500 running across both decks.
		expectVector(snapped.guides[0][0], [1000, -500, 400]);
		expectVector(snapped.guides[1][0], [-1000, -500, 400]);
		expectVector(snapped.guides[1][1], [2000, -500, 400]);
	});

	it("lines up stage elements turned a quarter turn, but not ones at an angle", () => {
		const still = riser("a", [0, 0, 0], [2000, 1000, 400]);
		// Turned a quarter, the 2 × 1 m deck is 1 m across and 2 m deep.
		const turned = {
			...riser("b", [1540, 700, 0], [2000, 1000, 400]),
			rotationDegrees: [0, 0, 90] as V3,
		};
		expectVector(snapMove([still, turned], ["b"], [0, 0, 0], PLAN, 150).delta, [-40, 0, 0]);
		const askew = { ...turned, rotationDegrees: [0, 0, 30] as V3 };
		expectVector(snapMove([still, askew], ["b"], [0, 0, 0], PLAN, 150).delta, [0, 0, 0]);
	});

	it("stands a stage element's feet on the top of another", () => {
		// A deck from a show made before the decks were generated stands on its feet by its name.
		const deck = base("a", {
			fixtureProfile: "Venue Stage Deck 2 × 1 m, Legs 0.4 m",
			sizeMillimetres: [2000, 1000, 440],
		});
		const moving = riser("b", [300, 0, 500], [1000, 1000, 400]);
		const snapped = snapMove([deck, moving], ["b"], [0, 0, -40], FRONT, 150);
		expectVector(snapped.delta, [0, 0, -60]);
		expect(snapped.targets[0][2]).toBe(440);
		// Seen from above, height is not the drag's to change.
		expect(snapMove([deck, moving], ["b"], [0, 5, 0], PLAN, 150).delta[2]).toBe(0);
	});

	it("hangs a curtain's rail under a pipe and lines its ends up with the next curtain", () => {
		const pipe = truss("p", [0, 0, 6000], [4000, 50, 50], 1);
		const drape = curtain("c", [100, 80, 2950]);
		expectVector(snapMove([pipe, drape], ["c"], [0, 0, 5], FRONT, 150).delta, [0, 0, 25]);
		const neighbour = curtain("n", [0, 0, 3000]);
		const next = curtain("m", [3040, 0, 3000]);
		expectVector(snapMove([neighbour, next], ["m"], [0, 10, 0], PLAN, 150).delta, [-40, 0, 0]);
	});

	it("hangs a lamp on the pipe its clamp reaches across, however far below it started", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		const half = trussParts(290, 4).spacing / 2;
		// The lamp is a metre square, so its clamp reaches the near chord from 100 mm away.
		const hung = lamp("l", [500, 100, 4800]);
		const near = snapMove([box, hung], ["l"], [0, 5, 0], PLAN, 150).delta;
		expectVector([near[0], near[1]], [0, half - 100]);
		// The whole point: a lamp on the floor goes up onto the truss rather than staying put.
		const floor = { ...hung, positionMillimetres: [500, 100, 0] as V3 };
		const risen = snapMove([box, floor], ["l"], [0, 5, 0], PLAN, 150).delta;
		expectVector([risen[0], risen[1]], [0, half - 100]);
		expect(risen[2]).toBeGreaterThan(4000);
	});

	it("leaves a lamp alone when its clamp is nowhere near a pipe on the page", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		const away = lamp("l", [500, 4000, 0]);
		expectVector(snapMove([box, away], ["l"], [0, 5, 0], PLAN, 150).delta, [0, 5, 0]);
	});

	it("leaves a fixture that hangs from nothing where the operator put it", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		// A hazer stands on the floor: its profile says so, and no pipe picks it up.
		const stands = {
			...lamp("h", [500, 100, 4800]),
			mounting: { hardware: "none" as const, centre: [0, 0, 0] as V3, halfExtent: [0, 0, 0] as V3, pipe: [0, 0, 0] as V3 },
		};
		expectVector(snapMove([box, stands], ["h"], [0, 5, 0], PLAN, 150).delta, [0, 5, 0]);
	});

	it("leaves a fixture whose profile declares no clip at all alone", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		const undeclared = base("u", { kind: "profile", positionMillimetres: [500, 100, 4800] });
		expectVector(snapMove([box, undeclared], ["u"], [0, 5, 0], PLAN, 150).delta, [0, 5, 0]);
	});

	it("moves several lamps rigidly onto a pipe, keeping the spacing between them", () => {
		const box = truss("t", [0, 0, 5000], [4000, 290, 290], 4);
		const one = lamp("a", [0, 100, 0]);
		const two = lamp("b", [1500, 100, 0]);
		const { delta } = snapMove([box, one, two], ["a", "b"], [0, 5, 0], PLAN, 150);
		// One correction for the pair: whatever it is, both move by it and stay 1500 apart.
		expect(delta[2]).toBeGreaterThan(4000);
		const movedOne = one.positionMillimetres[0] + delta[0];
		const movedTwo = two.positionMillimetres[0] + delta[0];
		expect(movedTwo - movedOne).toBe(1500);
	});

	it("lands a handrail on the outside edge of a stage element", () => {
		// A 2 x 1 m deck 600 mm high, standing on its own feet at the origin: its top perimeter is
		// 600 mm up, and its front edge runs along y = -500.
		const deck = riser("d", [0, 0, 0], [2000, 1000, 600]);
		const rail = railing("r", [0, -460, 600]);
		// Dragged in a plan view, the rail's foot line lands on the deck's front edge.
		expectVector(snapMove([deck, rail], ["r"], [0, -5, 0], PLAN, 150).delta, [0, -40, 0]);
	});

	it("leaves a handrail alone when no stage edge is within reach", () => {
		const deck = riser("d", [0, 0, 0], [2000, 1000, 600]);
		const away = railing("r", [0, -4000, 600]);
		expectVector(snapMove([deck, away], ["r"], [0, -5, 0], PLAN, 150).delta, [0, -5, 0]);
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
