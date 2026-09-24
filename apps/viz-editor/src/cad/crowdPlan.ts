/**
 * An audience as a plan draws people: seen from above, rows of heads staggered so the row behind
 * looks through the gaps in front; seen from the front or a side, a line of figures standing along
 * the width, the side views turned to face the stage.
 *
 * The plan fills the footprint it is given: a wider or deeper area holds more rows and more people
 * in each, at the spacing of a standing audience, rather than stretching the same few.
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

/**
 * How tall the people of an audience are, in millimetres: an adult crowd spreads between these,
 * around a typical 1.70 m, and a two-metre figure is the rare exception rather than the drawing.
 */
const SHORTEST = 1550;
const TALLEST = 1850;
const AVERAGE_HEIGHT = (SHORTEST + TALLEST) / 2;

/** How far apart the people of a standing audience are drawn, and the widest a plan mark gets. */
const PERSON_SPACING = 700;
const SHOULDERS = 450;

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
	const { columns, rows } = crowdGrid(w, h);
	const across = w / columns;
	const deep = h / rows;
	// Small enough in its cell that the stagger and the widest person stay inside the footprint.
	const personWidth = Math.min(SHOULDERS, across * 0.6, deep * 0.6);
	for (let row = 0; row < rows; row++) {
		for (let column = 0; column < columns; column++) {
			// Alternate rows sit an eighth of a step either way, so the row behind looks through the gaps.
			const x = -w / 2 + (column + 0.5 + (row % 2 ? 0.125 : -0.125)) * across;
			const { width: wide, height: along } = audiencePersonScale(
				row * columns + column,
				seed,
			);
			const scale = personWidth / outlineWidth;
			const y = -h / 2 + (row + 0.5) * deep - (outlineHeight * scale * along) / 2;
			polygons.push(
				...strokes.map((stroke) =>
					outlinePolygon(stroke, x, y, [scale * wide, scale * along], false),
				),
			);
		}
	}
	return polygons;
}

/** How many rows and people per row a top view of a footprint holds. */
export function crowdGrid(width: number, depth: number): { columns: number; rows: number } {
	const count = (length: number, most: number) =>
		Math.max(1, Math.min(most, Math.round(length / PERSON_SPACING)));
	return { columns: Math.max(2, count(width, 24)), rows: count(depth, 16) };
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
		const across = AVERAGE_HEIGHT * audiencePersonScale(index, seed).width;
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
 * Stable pseudo-random audience stature in millimetres for repeatable technical drawings: 1.55 to
 * 1.85 m, the same for the same person of the same crowd every redraw.
 */
export function audiencePersonHeight(index: number, seed = 0): number {
	return AVERAGE_HEIGHT * audiencePersonScale(index, seed).height;
}

/**
 * How much wider and taller than average one person in a crowd is drawn: 0.85 to 1.15 as wide,
 * and as tall as a stature between the shortest and the tallest of the audience.
 */
export function audiencePersonScale(
	index: number,
	seed = 0,
): { width: number; height: number } {
	const person = Math.max(0, Math.trunc(index));
	const stature = SHORTEST + (TALLEST - SHORTEST) * unitNoise(seed, person * 2);
	return {
		width: 0.85 + 0.3 * unitNoise(seed, person * 2 + 1),
		height: stature / AVERAGE_HEIGHT,
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
