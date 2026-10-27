/**
 * Rotation has to be visible on the page, and has to say the same thing the Visualizer does.
 *
 * A drawing is a flat slice, so these check where its two axes land rather than where any one
 * point does: the axes are what carry pitch, roll and foreshortening.
 */
import { describe, expect, it } from "vitest";
import { planTransform } from "./planGeometry";
import type { PlanGeometry, PlanPoint } from "./projection";
import { type CadEntity, projectPoint } from "./types";

const UNTURNED: PlanGeometry = {
	source: "typed",
	triangles: [],
	outlines: [],
	lines: [],
} as unknown as PlanGeometry;

function element(rotationDegrees: [number, number, number]): CadEntity {
	return {
		id: "one",
		logicalFixtureId: "one",
		kind: "venue",
		positionMillimetres: [0, 0, 0],
		rotationDegrees,
		sizeMillimetres: [1000, 1000, 1000],
	} as unknown as CadEntity;
}

/** Where the drawing's across and up axes land on the page, rounded past floating-point dust. */
function axes(
	rotationDegrees: [number, number, number],
	view: Parameters<typeof planTransform>[2],
	rotationQuarterTurns = 0,
	geometry: PlanGeometry = UNTURNED,
) {
	const transform = planTransform(
		{ ...element(rotationDegrees), kind: geometry.source === "model_drawing" ? "fixture" : "venue" },
		geometry,
		view,
		rotationQuarterTurns,
	);
	// Normalised past floating-point dust, and past negative zero, which is not a direction.
	const round = (point: readonly number[]) =>
		point.map((value) => (Math.round(value * 1000) || 0) / 1000);
	return { across: round(transform([1, 0])), up: round(transform([0, 1])) };
}

describe("an unturned element", () => {
	it("draws its slice exactly as it was built, in every view", () => {
		for (const view of [
			"top_down",
			"front_to_back",
			"back_to_front",
			"left_to_right",
			"right_to_left",
		] as const) {
			expect(axes([0, 0, 0], view)).toEqual({ across: [1, 0], up: [0, 1] });
		}
	});
});

describe("a physical venue slice rolled 180 degrees about X", () => {
	it("hangs the other way up in an elevation, as it does in the Visualizer", () => {
		// The operator's case: pointing down in 2D while pointing up in 3D, because the body
		// symbol never received the roll and only the direction indicator did.
		expect(axes([180, 0, 0], "front_to_back").up).toEqual([0, -1]);
	});

	it("is seen from its other side looking down", () => {
		expect(axes([180, 0, 0], "top_down").up).toEqual([0, -1]);
	});
});

describe("a quarter turn about Z", () => {
	it("turns the drawing on the page looking down", () => {
		expect(axes([0, 0, 90], "top_down")).toEqual({ across: [0, 1], up: [-1, 0] });
	});

	it("is visible in an elevation too, by foreshortening the run to nothing", () => {
		// A 4 m truss turned side-on is drawn as its end, not as a 4 m run it no longer covers.
		expect(axes([0, 0, 90], "front_to_back").across).toEqual([0, 0]);
	});
});

describe("the page's own quarter turns", () => {
	it("turn a drawing the same way they turn where things stand", () => {
		// `rotatePlane` takes [1, 0] to [0, -1], and a drawing has to follow its own position.
		// The old page angle turned the other way, so on a quarter-turned plan every symbol was
		// skewed against the layout it belonged to.
		expect(axes([0, 0, 0], "top_down", 1)).toEqual({ across: [0, -1], up: [1, 0] });
	});

	it("compose with the element's own turn rather than replacing it", () => {
		// A quarter turn one way and a 90-degree yaw the other cancel exactly.
		expect(axes([0, 0, 90], "top_down", 1)).toEqual({ across: [1, 0], up: [0, 1] });
	});

	it("keep a drawing square to the positions around it", () => {
		for (const quarterTurns of [0, 1, 2, 3, -1]) {
			const { across } = axes([0, 0, 0], "top_down", quarterTurns);
			// Where the show's +X ends up on the page, from the projection positions use.
			const position = projectPoint([1000, 0, 0], "top_down", quarterTurns);
			expect(across).toEqual(position.map((value) => Math.round(value / 1000) || 0));
		}
	});
});

describe("an imported model", () => {
	it("keeps the axes it was projected with, having already been turned in three dimensions", () => {
		const live = { ...UNTURNED, source: "live_model" } as unknown as PlanGeometry;
		expect(axes([180, 0, 90], "front_to_back", 0, live)).toEqual({
			across: [1, 0],
			up: [0, 1],
		});
	});
});

describe("a lamp whose drawing is read per side", () => {
	// What `modelDrawingGeometry` and the projection-sheet path report for a quarter-turned lamp:
	// the drawing is already the side the yaw turns toward the view.
	const drawn = (yawDegrees: number): PlanGeometry =>
		({
			...UNTURNED,
			source: "model_drawing",
			yawQuarterTurnsShown: Math.round(yawDegrees / 90),
		}) as unknown as PlanGeometry;

	it("shows that side at full width in the elevation each quarter turn brings it to", () => {
		// The operator's case: a lamp yawed 90 degrees is side-on to the front elevation, and the
		// drawing read there is already its side view, so turning it again would leave a bare line.
		const quarters: [number, Parameters<typeof planTransform>[2]][] = [
			[90, "front_to_back"],
			[180, "front_to_back"],
			[270, "front_to_back"],
			[-90, "front_to_back"],
			[90, "left_to_right"],
			[180, "left_to_right"],
			[270, "left_to_right"],
			[90, "right_to_left"],
			[180, "back_to_front"],
		];
		for (const [yaw, view] of quarters) {
			expect(axes([0, 0, yaw], view, 0, drawn(yaw))).toEqual({
				across: [1, 0],
				up: [0, 1],
			});
		}
	});

	it("still turns the whole yaw looking down, where one drawing serves every heading", () => {
		// A top view reads no other side, so it reports none shown and turns by the yaw itself.
		expect(
			axes([0, 0, 90], "top_down", 0, {
				...UNTURNED,
				source: "model_drawing",
			} as unknown as PlanGeometry),
		).toEqual({ across: [0, 1], up: [-1, 0] });
	});

	it("keeps the selected side full width when yaw lies out of the elevation plane", () => {
		expect(axes([0, 0, 45], "front_to_back", 0, drawn(45)).across).toEqual([1, 0]);
	});

	it("does not flip a canonical front drawing for out-of-plane roll", () => {
		expect(axes([180, 0, 90], "front_to_back", 0, drawn(90)).up).toEqual([0, 1]);
	});
});

describe("canonical lamp drawings", () => {
	const sheet = { ...UNTURNED, source: "model_drawing" } as PlanGeometry;
	function lampAxes(rotation: [number, number, number], view: Parameters<typeof planTransform>[2], quarterTurns = 0) {
		const transform = planTransform({ ...element(rotation), kind: "fixture" }, sheet, view, quarterTurns);
		const round = (point: PlanPoint) => point.map((value) => (Math.round(value * 1000000) || 0) / 1000000);
		return { across: round(transform([1, 0])), up: round(transform([0, 1])) };
	}

	it("keeps the ELP top drawing full size despite its 65.9 degree pitch and compound aim", () => {
		expect(lampAxes([65.9, -13.5, 180], "top_down")).toEqual(lampAxes([0, 0, 180], "top_down"));
	});

	it("ignores rotations out of each view's plane", () => {
		for (const view of ["top_down", "front_to_back", "back_to_front", "left_to_right", "right_to_left"] as const) {
			const normal = view === "top_down" ? 2 : view === "front_to_back" || view === "back_to_front" ? 1 : 0;
			const rotation: [number, number, number] = [67, 38, 24];
			const inPlane: [number, number, number] = [0, 0, 0];
			inPlane[normal] = rotation[normal];
			expect(lampAxes(rotation, view)).toEqual(lampAxes(inPlane, view));
		}
	});

	it("preserves lengths and perpendicular axes at every in-plane angle and page quarter turn", () => {
		for (const view of ["top_down", "front_to_back", "back_to_front", "left_to_right", "right_to_left"] as const) {
			for (const quarterTurns of [0, 1, 2, 3]) {
				const { across, up } = lampAxes([67, 38, 24], view, quarterTurns);
				expect(Math.hypot(...across)).toBeCloseTo(1, 3);
				expect(Math.hypot(...up)).toBeCloseTo(1, 3);
				expect(across[0] * up[0] + across[1] * up[1]).toBeCloseTo(0, 3);
			}
		}
	});
});
