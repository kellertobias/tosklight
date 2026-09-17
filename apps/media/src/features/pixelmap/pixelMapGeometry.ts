// Moving and resizing a rectangle on the canvas, in canvas fractions.
//
// Dragging on the picture and nudging with the keyboard both end here, so a shape moved either way
// lands on the same grid and never leaves the canvas.

export type CanvasPoint = { x: number; y: number };
export type CanvasShape = { start: CanvasPoint; end: CanvasPoint };

/** What a press on the picture grabbed: the whole shape, or one of its corners. */
export type ShapeHandle =
	| "move"
	| "top-left"
	| "top-right"
	| "bottom-left"
	| "bottom-right";

export const RESIZE_HANDLES: {
	handle: Exclude<ShapeHandle, "move">;
	label: string;
}[] = [
	{ handle: "top-left", label: "top-left corner" },
	{ handle: "top-right", label: "top-right corner" },
	{ handle: "bottom-left", label: "bottom-left corner" },
	{ handle: "bottom-right", label: "bottom-right corner" },
];

/// The smallest a dragged shape may become, so a corner pulled past its opposite corner leaves a
/// shape that can still be seen and grabbed rather than one covering none of the canvas.
export const MIN_SIZE = 0.01;

/** Fractions are kept to a thousandth, which is finer than any pixel an output can show. */
function snap(value: number): number {
	return Math.round(value * 1000) / 1000;
}

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}

/** The shape with its start at the top left, whichever way it was stored. */
export function normalized<T extends CanvasShape>(shape: T): T {
	return {
		...shape,
		start: {
			x: Math.min(shape.start.x, shape.end.x),
			y: Math.min(shape.start.y, shape.end.y),
		},
		end: {
			x: Math.max(shape.start.x, shape.end.x),
			y: Math.max(shape.start.y, shape.end.y),
		},
	};
}

/**
 * The shape after a drag of `dx`, `dy` canvas fractions on `handle`.
 *
 * A move keeps the shape's size and stops at the canvas edge. A corner moves only itself, stops at
 * the canvas edge, and never crosses the opposite corner.
 */
export function dragShape<T extends CanvasShape>(
	shape: T,
	handle: ShapeHandle,
	dx: number,
	dy: number,
): T {
	const box = normalized(shape);
	let { x: left, y: top } = box.start;
	let { x: right, y: bottom } = box.end;
	if (handle === "move") {
		const width = right - left;
		const height = bottom - top;
		left = clamp(left + dx, 0, 1 - width);
		top = clamp(top + dy, 0, 1 - height);
		right = left + width;
		bottom = top + height;
	} else {
		if (handle === "top-left" || handle === "bottom-left")
			left = clamp(left + dx, 0, right - MIN_SIZE);
		else right = clamp(right + dx, left + MIN_SIZE, 1);
		if (handle === "top-left" || handle === "top-right")
			top = clamp(top + dy, 0, bottom - MIN_SIZE);
		else bottom = clamp(bottom + dy, top + MIN_SIZE, 1);
	}
	return {
		...shape,
		start: { x: snap(left), y: snap(top) },
		end: { x: snap(right), y: snap(bottom) },
	};
}

/** Whether two shapes cover the same part of the canvas. */
export function sameArea(left: CanvasShape, right: CanvasShape): boolean {
	return (
		left.start.x === right.start.x &&
		left.start.y === right.start.y &&
		left.end.x === right.end.x &&
		left.end.y === right.end.y
	);
}

/// The drag a key asks for: arrows move the shape by a hundredth, or a tenth with Shift; with Alt
/// they move the bottom-right corner instead, so a shape can be resized without a pointer.
export function keyDrag(key: {
	key: string;
	shiftKey: boolean;
	altKey: boolean;
}): { handle: ShapeHandle; dx: number; dy: number } | null {
	const step = key.shiftKey ? 0.1 : 0.01;
	const direction: Record<string, [number, number]> = {
		ArrowLeft: [-1, 0],
		ArrowRight: [1, 0],
		ArrowUp: [0, -1],
		ArrowDown: [0, 1],
	};
	const found = direction[key.key];
	if (!found) return null;
	return {
		handle: key.altKey ? "bottom-right" : "move",
		dx: found[0] * step,
		dy: found[1] * step,
	};
}
