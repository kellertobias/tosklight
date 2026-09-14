/**
 * A hoist chain in plan and elevation, drawn link by link.
 *
 * The proportions are those of the round-link chain a stage hoist lifts: 7 mm wire, links 35 mm
 * long and 24 mm wide outside with a 21 × 10 mm opening. Each link passes through the opening of
 * the one before it, turned a quarter, so an elevation alternates a link seen face-on (its outside
 * and its opening) with one seen edge-on (a 7 mm bar) and repeats every 35 − 2 × 7 = 21 mm. A
 * hoist hangs from its hook at one end; the other end is shackled to a steelflex wrapped round a
 * truss chord.
 */
import type { PlanGeometry, PlanLine, PlanPoint, PlanTriangle } from "./projection";
import type { CadChainMode, CadViewDirection } from "./types";

const FILL: [number, number, number] = [0.38, 0.42, 0.47];

/** Wire diameter of one link. */
export const CHAIN_WIRE = 7;
/** Outside length and width of one link. */
export const CHAIN_LINK_LENGTH = 35;
export const CHAIN_LINK_WIDTH = 24;
/** Centre-to-centre distance of neighbouring links: each sits a wire deep inside the other. */
export const CHAIN_PITCH = CHAIN_LINK_LENGTH - 2 * CHAIN_WIRE;

/** The hoist body: width, height and depth. */
const HOIST = { width: 280, height: 420, depth: 260 } as const;
/** The suspension hook above (or below) the hoist, and the plate it swivels in. */
const HOOK_LENGTH = 140;
const HOOK_PLATE = { width: 70, height: 24 } as const;
/** A bow shackle: outside, opening, and how far its pin passes into the end link. */
const SHACKLE = { width: 26, length: 44, wire: 7 } as const;
/** The truss chord the steelflex wraps, and the wire's own diameter. */
const CHORD_RADIUS = 25;
const STEELFLEX = 6;
/**
 * Each steelflex leg's angle from the chain's line: half the 45° between the two legs, the same
 * angle the Visualizer rigs them at.
 */
export const STEELFLEX_LEG_DEGREES = 22.5;

interface Shapes {
	outlines: PlanPoint[][];
	lines: PlanLine[];
	/** Convex areas that mask what is behind and make the object pickable. */
	masks: PlanPoint[][];
}

export function chainPlan(
	sizeMillimetres: readonly [number, number, number],
	view: CadViewDirection,
	mode: CadChainMode = "motor_top",
): PlanGeometry {
	const height = Math.max(CHAIN_LINK_LENGTH, sizeMillimetres[2]);
	if (view === "top_down") return finish(topView(mode));
	const side = view === "left_to_right" || view === "right_to_left";
	const shapes = elevation(height, side, mode);
	const mirror = view === "back_to_front" || view === "right_to_left";
	return finish(mirror ? transform(shapes, ([x, y]) => [-x, y]) : shapes);
}

/** From above: the two link orientations crossed, unless a hoist on top hides the chain. */
function topView(mode: CadChainMode): Shapes {
	if (mode === "motor_top") {
		const body = roundedRect(0, 0, HOIST.width, HOIST.depth, 30);
		return { outlines: [body], lines: [], masks: [body] };
	}
	const across = stadium(0, 0, CHAIN_LINK_WIDTH, CHAIN_WIRE);
	const along = stadium(0, 0, CHAIN_WIRE, CHAIN_LINK_WIDTH);
	return { outlines: [across, along], lines: [], masks: [across, along] };
}

/**
 * An elevation, laid out as for a hoist on top from the top of the object down; a hoist at the
 * bottom is the same drawing turned upside down.
 */
function elevation(height: number, side: boolean, mode: CadChainMode): Shapes {
	const top = height / 2;
	const bottom = -height / 2;
	const shapes: Shapes = { outlines: [], lines: [], masks: [] };
	if (mode === "plain") {
		links(shapes, top, bottom, side);
		return shapes;
	}
	const hoistBottom = hoist(shapes, top, side);
	const chainEnd = steelflex(shapes, bottom);
	links(shapes, hoistBottom, chainEnd, side);
	return mode === "motor_bottom" ? transform(shapes, ([x, y]) => [x, -y]) : shapes;
}

/** Links from `from` down to `to`, centred in that run; the first link is face-on in front. */
function links(shapes: Shapes, from: number, to: number, side: boolean) {
	const run = Math.max(CHAIN_LINK_LENGTH, from - to);
	const count = Math.max(1, Math.floor((run - CHAIN_LINK_LENGTH) / CHAIN_PITCH) + 1);
	const used = CHAIN_LINK_LENGTH + (count - 1) * CHAIN_PITCH;
	const first = (from + to) / 2 + used / 2 - CHAIN_LINK_LENGTH / 2;
	for (let index = 0; index < count; index++) {
		const y = first - index * CHAIN_PITCH;
		const faceOn = index % 2 === (side ? 1 : 0);
		if (faceOn) {
			const outside = stadium(0, y, CHAIN_LINK_WIDTH, CHAIN_LINK_LENGTH);
			const opening = stadium(
				0,
				y,
				CHAIN_LINK_WIDTH - 2 * CHAIN_WIRE,
				CHAIN_LINK_LENGTH - 2 * CHAIN_WIRE,
			);
			shapes.outlines.push(outside, opening);
			// Only the wire is solid: the opening stays hollow, so the next link shows through it.
			// Both stadiums have their radius at half their width, so their points pair up round the
			// ring, and each pair of pairs is a small convex quad of wire.
			outside.forEach((point, index) => {
				const next = (index + 1) % outside.length;
				shapes.masks.push([point, outside[next], opening[next], opening[index]]);
			});
		} else {
			const edge = stadium(0, y, CHAIN_WIRE, CHAIN_LINK_LENGTH);
			shapes.outlines.push(edge);
			shapes.masks.push(edge);
		}
	}
}

/** The hoist hanging from its hook at `top`; returns where its chain leaves the body. */
function hoist(shapes: Shapes, top: number, side: boolean): number {
	const width = side ? HOIST.depth : HOIST.width;
	const plateTop = top - HOOK_LENGTH;
	const bodyTop = plateTop - HOOK_PLATE.height;
	const bodyBottom = bodyTop - HOIST.height;
	const body = roundedRect(0, (bodyTop + bodyBottom) / 2, width, HOIST.height, 24);
	const plate = roundedRect(0, plateTop - HOOK_PLATE.height / 2, HOOK_PLATE.width, HOOK_PLATE.height, 6);
	shapes.outlines.push(body, plate);
	shapes.masks.push(body, plate);
	// The gearbox joint and the motor's cooling fins read the body as a hoist, not a crate.
	const seam = bodyTop - HOIST.height * 0.38;
	shapes.lines.push({ points: [[-width / 2, seam], [width / 2, seam]] });
	for (let fin = 1; fin <= 4; fin++) {
		const y = seam - (HOIST.height * 0.62 * fin) / 5;
		shapes.lines.push({ points: [[-width * 0.36, y], [width * 0.36, y]] });
	}
	hook(shapes, plateTop, top, side);
	return bodyBottom;
}

/** A swivel hook: a shank up from the plate and a J opening to one side (edge-on from the side). */
function hook(shapes: Shapes, from: number, to: number, side: boolean) {
	const wire = 16;
	const length = to - from;
	if (side) {
		const bar = stadium(0, from + length / 2, wire, length);
		shapes.outlines.push(bar);
		shapes.masks.push(bar);
		return;
	}
	const radius = length * 0.24;
	const centre: PlanPoint = [radius, to - radius - wire / 2];
	// The centreline: straight up the shank, round the bowl, back down to the point.
	const path: PlanPoint[] = [[0, from]];
	const steps = 14;
	for (let step = 0; step <= steps; step++) {
		const angle = Math.PI - (step / steps) * Math.PI * 1.25;
		path.push([centre[0] + Math.cos(angle) * radius, centre[1] + Math.sin(angle) * radius]);
	}
	shapes.outlines.push(strokeOutline(path, wire));
	shapes.masks.push(roundedRect(0, from + wire, wire, wire * 2, 2));
}

/**
 * The steelflex at the bottom: a wrap round a truss chord at the bottom of the object, two legs
 * rising 45° apart to a shackle, and the shackle through the end link. Returns where the chain ends.
 */
function steelflex(shapes: Shapes, bottom: number): number {
	// The wrap is drawn along the wire's centreline, hugging the chord.
	const loop = CHORD_RADIUS + STEELFLEX / 2;
	const chord: PlanPoint = [0, bottom + loop];
	shapes.outlines.push(circle(chord[0], chord[1], CHORD_RADIUS, 20));
	shapes.masks.push(circle(chord[0], chord[1], loop, 20));
	const lean = (STEELFLEX_LEG_DEGREES * Math.PI) / 180;
	// Legs leaning ±lean from the chain's line meet this far above the chord's centre.
	const apex: PlanPoint = [0, chord[1] + loop / Math.sin(lean)];
	const wrap: PlanPoint[] = [];
	// The legs touch the wrap where its radius is square to them: π − lean on the left and lean on
	// the right. The wrap runs the long way round, under the chord, from one to the other.
	const start = Math.PI - lean;
	const sweep = Math.PI * 2 - 2 * (Math.PI / 2 - lean);
	for (let step = 0; step <= 24; step++) {
		const angle = start + (step / 24) * sweep;
		wrap.push([chord[0] + Math.cos(angle) * loop, chord[1] + Math.sin(angle) * loop]);
	}
	for (let index = 1; index < wrap.length; index++)
		shapes.lines.push({ points: [wrap[index - 1], wrap[index]] });
	shapes.lines.push(
		{ points: [wrap[0], apex] },
		{ points: [wrap[wrap.length - 1], apex] },
	);
	// The shackle's bow holds both eyes at the apex; its pin passes through the end link above.
	const shackleBottom = apex[1] - SHACKLE.wire;
	const shackleTop = shackleBottom + SHACKLE.length;
	const centre = (shackleTop + shackleBottom) / 2;
	const bow = stadium(0, centre, SHACKLE.width, SHACKLE.length);
	shapes.outlines.push(
		bow,
		stadium(0, centre, SHACKLE.width - 2 * SHACKLE.wire, SHACKLE.length - 2 * SHACKLE.wire),
	);
	shapes.masks.push(bow);
	const pin = shackleTop - SHACKLE.wire / 2;
	shapes.lines.push({ points: [[-SHACKLE.width / 2 - 6, pin], [SHACKLE.width / 2 + 6, pin]] });
	// The end link's bottom wire rests on the pin.
	return pin - CHAIN_WIRE / 2;
}

function finish(shapes: Shapes): PlanGeometry {
	const triangles: PlanTriangle[] = [];
	for (const mask of shapes.masks)
		for (let index = 1; index < mask.length - 1; index++)
			triangles.push({ points: [mask[0], mask[index], mask[index + 1]], color: FILL });
	return { source: "typed", triangles, outlines: shapes.outlines, lines: shapes.lines };
}

function transform(shapes: Shapes, map: (point: PlanPoint) => PlanPoint): Shapes {
	return {
		outlines: shapes.outlines.map((outline) => outline.map(map)),
		masks: shapes.masks.map((mask) => mask.map(map)),
		lines: shapes.lines.map((line) => ({
			...line,
			points: line.points.map(map) as PlanLine["points"],
		})),
	};
}

/** A rounded rectangle with a 50% corner radius: straight sides along its longer axis. */
export function stadium(x: number, y: number, width: number, height: number): PlanPoint[] {
	return roundedRect(x, y, width, height, Math.min(width, height) / 2);
}

function roundedRect(
	x: number,
	y: number,
	width: number,
	height: number,
	radius: number,
): PlanPoint[] {
	const r = Math.min(radius, width / 2, height / 2);
	const points: PlanPoint[] = [];
	const corners: [number, number, number][] = [
		[x + width / 2 - r, y + height / 2 - r, 0],
		[x - width / 2 + r, y + height / 2 - r, Math.PI / 2],
		[x - width / 2 + r, y - height / 2 + r, Math.PI],
		[x + width / 2 - r, y - height / 2 + r, (Math.PI * 3) / 2],
	];
	const steps = 6;
	for (const [cx, cy, start] of corners)
		for (let step = 0; step <= steps; step++) {
			const angle = start + (step / steps) * (Math.PI / 2);
			const point: PlanPoint = [cx + Math.cos(angle) * r, cy + Math.sin(angle) * r];
			const last = points[points.length - 1];
			if (!last || Math.hypot(last[0] - point[0], last[1] - point[1]) > 1e-6)
				points.push(point);
		}
	const [firstX, firstY] = points[0];
	const [lastX, lastY] = points[points.length - 1];
	if (Math.hypot(firstX - lastX, firstY - lastY) < 1e-6) points.pop();
	return points;
}

function circle(x: number, y: number, radius: number, segments: number): PlanPoint[] {
	return Array.from({ length: segments }, (_, index) => {
		const angle = (index / segments) * Math.PI * 2;
		return [x + Math.cos(angle) * radius, y + Math.sin(angle) * radius];
	});
}

/** The closed outline of a round-ended stroke `width` wide along an open path. */
function strokeOutline(path: readonly PlanPoint[], width: number): PlanPoint[] {
	const half = width / 2;
	const left: PlanPoint[] = [];
	const right: PlanPoint[] = [];
	path.forEach((point, index) => {
		const before = path[Math.max(0, index - 1)];
		const after = path[Math.min(path.length - 1, index + 1)];
		const dx = after[0] - before[0];
		const dy = after[1] - before[1];
		const length = Math.hypot(dx, dy) || 1;
		const normal: PlanPoint = [(-dy / length) * half, (dx / length) * half];
		left.push([point[0] + normal[0], point[1] + normal[1]]);
		right.push([point[0] - normal[0], point[1] - normal[1]]);
	});
	const tip = path[path.length - 1];
	const previous = path[path.length - 2];
	const angle = Math.atan2(tip[1] - previous[1], tip[0] - previous[0]);
	const cap: PlanPoint[] = Array.from({ length: 5 }, (_, step) => {
		const a = angle + Math.PI / 2 - ((step + 1) / 6) * Math.PI;
		return [tip[0] + Math.cos(a) * half, tip[1] + Math.sin(a) * half];
	});
	return [...left, ...cap, ...right.reverse()];
}
