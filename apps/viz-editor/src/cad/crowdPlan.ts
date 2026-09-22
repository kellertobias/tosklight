/**
 * An audience as a plan draws people: seen from above, rows of heads staggered so the row behind
 * looks through the gaps in front; seen from the front or a side, a line of figures standing along
 * the width, the side views turned to face the stage.
 *
 * Nobody in a crowd is average, so every person is scaled from a seed taken off the entity's id.
 * That keeps one audience varied while drawing it identically every redraw — a plan an operator
 * prints twice must come out the same both times, and two crowds side by side must not look like
 * the same people copied.
 */
import type { PlanPoint } from "./projection";
import { audienceOutlineFor, audienceStrokesFor } from "./audienceOutline";
import type { CadViewDirection } from "./types";

export interface CrowdPolygon {
	points: PlanPoint[];
	color: [number, number, number];
}

/** The mid grey a figure is drawn in, the same body tone the other typed symbols use. */
const BODY: [number, number, number] = [0.38, 0.42, 0.47];

/** The stature an audience outline is scaled from before each person's own variation. */
const AUDIENCE_HEIGHT = 1750;

export function crowdPlan(
	width: number,
	height: number,
	view: CadViewDirection,
	seed: number,
): CrowdPolygon[] {
	const w = Math.max(600, width);
	if (view !== "top_down") return elevationCrowd(w, view, seed);

	const h = Math.max(500, height);
	const polygons: CrowdPolygon[] = [];
	const outline = audienceOutlineFor("top");
	const strokes = audienceStrokesFor("top");
	const outlineWidth = outlineRange(outline, 0);
	const outlineHeight = outlineRange(outline, 1);
	for (let row = 0; row < 4; row++) {
		for (let column = 0; column < 6; column++) {
			const x = -w * 0.4 + (column / 5) * w * 0.8 + (row % 2 ? w * 0.04 : 0);
			const personWidth = Math.min(w * 0.07, h * 0.1);
			const { width: across, height: along } = audiencePersonScale(
				row * 6 + column,
				seed,
			);
			const scale = personWidth / outlineWidth;
			const y =
				-h * 0.38 + (row / 3) * h * 0.76 - (outlineHeight * scale * along) / 2;
			polygons.push(
				...strokes.map((stroke) =>
					outlinePolygon(stroke, x, y, [scale * across, scale * along], false),
				),
			);
		}
	}
	return polygons;
}

function elevationCrowd(
	width: number,
	view: CadViewDirection,
	seed: number,
): CrowdPolygon[] {
	const count = Math.max(6, Math.min(14, Math.round(width / 500)));
	const spacing = width / count;
	const side = view === "left_to_right" || view === "right_to_left";
	// Seen from house left the stage is on the left, so the audience faces that way.
	const mirror = view === "left_to_right";
	const outlineView = side ? "side" : "front";
	const strokes = audienceStrokesFor(outlineView);
	const polygons: CrowdPolygon[] = [];
	for (let index = 0; index < count; index++) {
		const height = audiencePersonHeight(index, seed);
		const across = AUDIENCE_HEIGHT * audiencePersonScale(index, seed).width;
		const x = -width / 2 + spacing * (index + 0.5);
		polygons.push(
			...strokes.map((stroke) =>
				outlinePolygon(stroke, x, 0, [across, height], false, mirror),
			),
		);
	}
	return polygons;
}

function outlinePolygon(
	outline: readonly PlanPoint[],
	x: number,
	y: number,
	scale: number | readonly [number, number],
	centerVertically: boolean,
	mirror = false,
): CrowdPolygon {
	const [scaleX, scaleY] = typeof scale === "number" ? [scale, scale] : scale;
	const yOffset = centerVertically
		? (outlineRange(outline, 1) * scaleY) / 2
		: 0;
	return {
		color: BODY,
		points: outline.map(([pointX, pointY]) => [
			x + pointX * scaleX * (mirror ? -1 : 1),
			y + pointY * scaleY - yOffset,
		]),
	};
}

function outlineRange(outline: readonly PlanPoint[], axis: 0 | 1): number {
	const values = outline.map((point) => point[axis]);
	return Math.max(...values) - Math.min(...values);
}

/**
 * Stable pseudo-random audience stature in millimetres for repeatable technical drawings: 0.85 to
 * 1.15 of an average person, the same for the same person of the same crowd every redraw.
 */
export function audiencePersonHeight(index: number, seed = 0): number {
	return AUDIENCE_HEIGHT * audiencePersonScale(index, seed).height;
}

/** How much wider and taller than average one person in a crowd is drawn, each 0.85 to 1.15. */
export function audiencePersonScale(
	index: number,
	seed = 0,
): { width: number; height: number } {
	const person = Math.max(0, Math.trunc(index));
	return {
		width: 0.85 + 0.3 * unitNoise(seed, person * 2 + 1),
		height: 0.85 + 0.3 * unitNoise(seed, person * 2),
	};
}

/** A stable seed from an entity id, so every crowd varies differently yet repeatably. */
export function seedOf(id: string): number {
	let hash = 0x811c9dc5;
	for (let index = 0; index < id.length; index++) {
		hash ^= id.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193);
	}
	return hash >>> 0;
}

/** A well-mixed number in [0, 1) from a seed and a counter. */
function unitNoise(seed: number, counter: number): number {
	let value = (seed ^ Math.imul(counter + 1, 0x9e3779b1)) >>> 0;
	value = Math.imul(value ^ (value >>> 16), 0x85ebca6b);
	value = Math.imul(value ^ (value >>> 13), 0xc2b2ae35);
	return ((value ^ (value >>> 16)) >>> 0) / 4294967296;
}
