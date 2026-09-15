/**
 * The Placement Assistant's arrangements: where each selected element stands when the selection is
 * laid out along a line, in a grid or around a circle.
 *
 * Every measurement is in metres in plan axes — X across, Y deep, Z up — and the result is whole
 * millimetres, one position per element in the order they were selected.
 */

export interface PlanPosition {
	x: number;
	y: number;
	z: number;
}

export type AssistantLayout =
	| { shape: "line"; start: PlanPosition; end: PlanPosition }
	| { shape: "grid"; start: PlanPosition; columns: number; spacingX: number; spacingY: number }
	| {
			shape: "circle";
			centre: PlanPosition;
			radius: number;
			/** Degrees from +X towards +Y where the first element stands. */
			startAngle: number;
			/** Degrees the elements cover; 360 or more spaces them evenly around the whole circle. */
			arc: number;
	  };

export type AssistantShape = AssistantLayout["shape"];

function millimetres({ x, y, z }: PlanPosition): PlanPosition {
	// `+ 0` turns a rounded -0 into 0.
	const whole = (metres: number) => Math.round(metres * 1000) + 0;
	return { x: whole(x), y: whole(y), z: whole(z) };
}

function linePositions(start: PlanPosition, end: PlanPosition, count: number) {
	return Array.from({ length: count }, (_, index) => {
		const share = count > 1 ? index / (count - 1) : 0;
		return {
			x: start.x + (end.x - start.x) * share,
			y: start.y + (end.y - start.y) * share,
			z: start.z + (end.z - start.z) * share,
		};
	});
}

/** Row by row from the start: across in X, then the next row one Y spacing deeper. */
function gridPositions(
	start: PlanPosition,
	columns: number,
	spacingX: number,
	spacingY: number,
	count: number,
) {
	const perRow = Math.max(1, Math.floor(columns));
	return Array.from({ length: count }, (_, index) => ({
		x: start.x + (index % perRow) * spacingX,
		y: start.y + Math.floor(index / perRow) * spacingY,
		z: start.z,
	}));
}

function circlePositions(
	centre: PlanPosition,
	radius: number,
	startAngle: number,
	arc: number,
	count: number,
) {
	// A whole circle would put the last element on the first, so it is divided by the count instead.
	const whole = Math.abs(arc) >= 360;
	const step = whole ? (Math.sign(arc) * 360) / count : count > 1 ? arc / (count - 1) : 0;
	return Array.from({ length: count }, (_, index) => {
		const radians = ((startAngle + step * index) * Math.PI) / 180;
		return {
			x: centre.x + radius * Math.cos(radians),
			y: centre.y + radius * Math.sin(radians),
			z: centre.z,
		};
	});
}

/** One position in millimetres for each of `count` elements. */
export function assistantPositions(layout: AssistantLayout, count: number): PlanPosition[] {
	const positions =
		layout.shape === "line"
			? linePositions(layout.start, layout.end, count)
			: layout.shape === "grid"
				? gridPositions(layout.start, layout.columns, layout.spacingX, layout.spacingY, count)
				: circlePositions(layout.centre, layout.radius, layout.startAngle, layout.arc, count);
	return positions.map(millimetres);
}
