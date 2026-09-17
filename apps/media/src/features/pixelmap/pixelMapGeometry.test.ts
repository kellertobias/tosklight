import { describe, expect, it } from "vitest";
import { dragShape, keyDrag, MIN_SIZE, normalized } from "./pixelMapGeometry";

const shape = {
	id: "zone",
	start: { x: 0.2, y: 0.3 },
	end: { x: 0.6, y: 0.5 },
};

describe("dragging a shape on the canvas", () => {
	it("moves the whole shape and keeps everything else", () => {
		expect(dragShape(shape, "move", 0.1, -0.1)).toEqual({
			id: "zone",
			start: { x: 0.3, y: 0.2 },
			end: { x: 0.7, y: 0.4 },
		});
	});

	it("stops a move at the canvas edge without changing the size", () => {
		expect(dragShape(shape, "move", 5, 5)).toMatchObject({
			start: { x: 0.6, y: 0.8 },
			end: { x: 1, y: 1 },
		});
		expect(dragShape(shape, "move", -5, -5)).toMatchObject({
			start: { x: 0, y: 0 },
			end: { x: 0.4, y: 0.2 },
		});
	});

	it("moves only the dragged corner", () => {
		expect(dragShape(shape, "top-left", -0.1, -0.1)).toMatchObject({
			start: { x: 0.1, y: 0.2 },
			end: { x: 0.6, y: 0.5 },
		});
		expect(dragShape(shape, "top-right", 0.1, 0.1)).toMatchObject({
			start: { x: 0.2, y: 0.4 },
			end: { x: 0.7, y: 0.5 },
		});
		expect(dragShape(shape, "bottom-left", 0.1, 0.1)).toMatchObject({
			start: { x: 0.3, y: 0.3 },
			end: { x: 0.6, y: 0.6 },
		});
		expect(dragShape(shape, "bottom-right", 1, 1)).toMatchObject({
			start: { x: 0.2, y: 0.3 },
			end: { x: 1, y: 1 },
		});
	});

	it("never lets a corner cross the opposite one", () => {
		const collapsed = dragShape(shape, "bottom-right", -1, -1);
		expect(collapsed.end.x - collapsed.start.x).toBeCloseTo(MIN_SIZE, 6);
		expect(collapsed.end.y - collapsed.start.y).toBeCloseTo(MIN_SIZE, 6);
		expect(collapsed.start).toEqual(shape.start);
	});

	it("snaps to a thousandth of the canvas", () => {
		expect(dragShape(shape, "move", 0.012345, 0)).toMatchObject({
			start: { x: 0.212, y: 0.3 },
			end: { x: 0.612, y: 0.5 },
		});
	});

	it("reads a shape stored end-first from its top left", () => {
		const reversed = { start: { x: 0.6, y: 0.5 }, end: { x: 0.2, y: 0.3 } };
		expect(normalized(reversed)).toEqual({
			start: { x: 0.2, y: 0.3 },
			end: { x: 0.6, y: 0.5 },
		});
		expect(dragShape(reversed, "move", 0.1, 0)).toEqual({
			start: { x: 0.3, y: 0.3 },
			end: { x: 0.7, y: 0.5 },
		});
	});
});

describe("nudging a shape with the keyboard", () => {
	const key = (name: string, shiftKey = false, altKey = false) =>
		keyDrag({ key: name, shiftKey, altKey });

	it("moves by a hundredth, or a tenth with Shift", () => {
		expect(key("ArrowLeft")).toEqual({ handle: "move", dx: -0.01, dy: 0 });
		expect(key("ArrowDown", true)).toEqual({
			handle: "move",
			dx: 0,
			dy: 0.1,
		});
	});

	it("resizes from the bottom-right corner with Alt", () => {
		expect(key("ArrowRight", false, true)).toEqual({
			handle: "bottom-right",
			dx: 0.01,
			dy: 0,
		});
	});

	it("ignores every other key", () => {
		expect(key("Enter")).toBeNull();
		expect(key(" ")).toBeNull();
	});
});
