import { describe, expect, it } from "vitest";
import {
	boundsOf,
	entityBounds,
	marqueeCatches,
	marqueeMode,
} from "./marqueeSelection";

function entity(
	position: [number, number, number],
	size: [number, number, number],
	rotation: [number, number, number] = [0, 0, 0],
) {
	return {
		positionMillimetres: position,
		rotationDegrees: rotation,
		sizeMillimetres: size,
	};
}

describe("marquee direction", () => {
	it("takes only what fits inside when dragged left to right", () => {
		expect(marqueeMode(100, 400)).toBe("enclose");
	});

	it("takes everything it touches when dragged right to left", () => {
		expect(marqueeMode(400, 100)).toBe("touch");
	});

	it("treats a rectangle with no width as an enclosing one", () => {
		expect(marqueeMode(200, 200)).toBe("enclose");
	});
});

describe("what a marquee catches", () => {
	const marquee = boundsOf([0, 0], [1_000, 1_000]);
	const inside = entityBounds(entity([500, -500, 0], [100, 100, 100]), "top_down");
	const across = entityBounds(
		entity([900, -500, 0], [400, 100, 100]),
		"top_down",
	);
	const outside = entityBounds(
		entity([3_000, -500, 0], [100, 100, 100]),
		"top_down",
	);

	it("encloses only what is wholly within the rectangle", () => {
		expect(marqueeCatches(inside, marquee, "enclose")).toBe(true);
		// Half of this one hangs out past the right edge.
		expect(marqueeCatches(across, marquee, "enclose")).toBe(false);
		expect(marqueeCatches(outside, marquee, "enclose")).toBe(false);
	});

	it("touches anything the rectangle overlaps at all", () => {
		expect(marqueeCatches(inside, marquee, "touch")).toBe(true);
		expect(marqueeCatches(across, marquee, "touch")).toBe(true);
		expect(marqueeCatches(outside, marquee, "touch")).toBe(false);
	});

	it("counts an entity sharing only an edge as touched, not enclosed", () => {
		const edge = entityBounds(entity([1_100, -500, 0], [200, 100, 100]), "top_down");
		expect(marqueeCatches(edge, marquee, "touch")).toBe(true);
		expect(marqueeCatches(edge, marquee, "enclose")).toBe(false);
	});
});

describe("the rectangle an entity covers", () => {
	it("covers the run a turned truss is drawn along", () => {
		const straight = entityBounds(
			entity([0, 0, 0], [4_000, 200, 200]),
			"top_down",
		);
		expect(straight.maximum[0] - straight.minimum[0]).toBeCloseTo(4_000);
		const turned = entityBounds(
			entity([0, 0, 0], [4_000, 200, 200], [0, 90, 0]),
			"top_down",
		);
		// Turned a quarter turn it runs the other way, so the plot must see it that way too.
		expect(turned.maximum[0] - turned.minimum[0]).toBeCloseTo(200);
		expect(turned.maximum[1] - turned.minimum[1]).toBeCloseTo(4_000);
	});

	it("covers the height of an element seen from the side", () => {
		const side = entityBounds(
			entity([0, 0, 3_000], [1_000, 500, 2_000]),
			"left_to_right",
		);
		expect(side.maximum[1] - side.minimum[1]).toBeCloseTo(2_000);
		expect(side.maximum[0] - side.minimum[0]).toBeCloseTo(500);
	});
});
