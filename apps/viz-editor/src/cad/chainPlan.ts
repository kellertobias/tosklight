/**
 * A hoist chain in plan and elevation, drawn link by link.
 *
 * The proportions are those of the round-link chain a stage hoist lifts: 7 mm wire, links 35 mm
 * long and 24 mm wide outside with a 21 × 10 mm opening. Each link passes through the opening of
 * the one before it, turned a quarter, so an elevation alternates a link seen face-on (its outside
 * and its opening) with one seen edge-on (a 7 mm bar) and repeats every 35 − 2 × 7 = 21 mm.
 *
 * A hoist hangs from its hook at one end. The other end is fixed by what it hangs from: a steelflex
 * wrapped round a three- or four-point truss's chord, a flange clamped round a pipe, or nothing but
 * a shackle made fast to the steel. Every fixing ends in a bow shackle whose bolt runs through the
 * last link, and the chain is laid out from that bolt so its last link bears on it.
 *
 * Nothing hidden is drawn: parts are laid down back to front and a later part hides what it covers.
 */
import { type DrawnPart, hideCoveredEdges } from "./hiddenLines";
import type { PlanGeometry, PlanLine, PlanPoint, PlanTriangle } from "./projection";
import type { CadChainAnchor, CadChainMode, CadViewDirection } from "./types";

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
/**
 * A 2 t bow shackle: a U of 13 mm wire, 21 mm wide and 48 mm long inside, closed by a 16 mm bolt
 * through the ends of its walls that stands 10 mm proud of each wall.
 */
export const SHACKLE = { wire: 13, insideWidth: 21, insideLength: 48, bolt: 16, overhang: 10 } as const;
/** A truss chord's radius, and the pipe a flange clamps. */
const CHORD_RADIUS = 25;
const PIPE_RADIUS = 24;
/** The steelflex sling's own thickness. */
export const STEELFLEX_WIDTH = 22;
/**
 * Each steelflex leg's angle from the chain's line: half the 45° between the two legs, the same
 * angle the Visualizer rigs them at.
 */
export const STEELFLEX_LEG_DEGREES = 22.5;
/** A flange: the clamp's flat steel, and the eye plate standing up from it to the shackle. */
const FLANGE = { clamp: 8, plateWidth: 40, plateLength: 42, eye: 10 } as const;

/**
 * Depth order, back to front: a later layer hides what an earlier one draws behind it. The last
 * link's near wire passes in front of the bolt it hangs on; the shackle's walls stand either side of
 * that link, in front of it where it is seen face-on; the hoist is in front of the chain it takes in.
 */
const FACE_ON_LINKS = 0;
const FIXING = 1;
const BOLT = 2;
const EDGE_ON_LINKS = 3;
const HOIST_LAYER = 4;
const SHACKLE_NEAR = 5;

interface Shapes {
	parts: { layer: number; part: DrawnPart }[];
}

export function chainPlan(
	sizeMillimetres: readonly [number, number, number],
	view: CadViewDirection,
	mode: CadChainMode = "motor_top",
	anchor: CadChainAnchor = "steelflex",
): PlanGeometry {
	const height = Math.max(CHAIN_LINK_LENGTH, sizeMillimetres[2]);
	if (view === "top_down") return finish(topView(mode));
	const side = view === "left_to_right" || view === "right_to_left";
	const shapes = elevation(height, side, mode, anchor);
	// Drawn for a view with upstage on the right; left to right (house left) has it on the left.
	const mirror = view === "back_to_front" || view === "left_to_right";
	return finish(mirror ? transform(shapes, ([x, y]) => [-x, y]) : shapes);
}

/** From above: the two link orientations crossed, unless a hoist on top hides the chain. */
function topView(mode: CadChainMode): Shapes {
	const shapes: Shapes = { parts: [] };
	if (mode === "motor_top") {
		solid(shapes, HOIST_LAYER, [roundedRect(0, 0, HOIST.width, HOIST.depth, 30)]);
		return shapes;
	}
	// The top link hangs face-on to the front, so from above it runs across, over the link below.
	solid(shapes, FACE_ON_LINKS, [stadium(0, 0, CHAIN_WIRE, CHAIN_LINK_WIDTH)]);
	solid(shapes, EDGE_ON_LINKS, [stadium(0, 0, CHAIN_LINK_WIDTH, CHAIN_WIRE)]);
	return shapes;
}

/**
 * An elevation, laid out as for a hoist on top from the top of the object down; a hoist at the
 * bottom is the same drawing turned upside down.
 */
function elevation(
	height: number,
	side: boolean,
	mode: CadChainMode,
	anchor: CadChainAnchor,
): Shapes {
	const top = height / 2;
	const bottom = -height / 2;
	const shapes: Shapes = { parts: [] };
	if (mode === "plain") {
		const count = Math.max(1, Math.floor((height - CHAIN_LINK_LENGTH) / CHAIN_PITCH) + 1);
		const used = CHAIN_LINK_LENGTH + (count - 1) * CHAIN_PITCH;
		// A chain with nothing at either end hangs centred, its top link face-on in front.
		for (let index = 0; index < count; index++)
			link(shapes, top - (height - used) / 2 - CHAIN_LINK_LENGTH / 2 - index * CHAIN_PITCH, index % 2 === (side ? 1 : 0));
		return shapes;
	}
	const hoistBottom = hoist(shapes, top, side);
	const bolt = fixing(shapes, bottom, side, anchor);
	// The last link's end wire bears on the bolt, and the chain climbs from there into the hoist,
	// whose body hides however far the top link reaches inside it.
	const chainEnd = bolt - SHACKLE.bolt / 2 - CHAIN_WIRE;
	const count = Math.max(1, Math.ceil((hoistBottom - chainEnd - CHAIN_LINK_LENGTH) / CHAIN_PITCH) + 1);
	for (let fromEnd = 0; fromEnd < count; fromEnd++) {
		// The bolt runs across the front, so the link on it is edge-on there and face-on from the side.
		const faceOn = (fromEnd % 2 === 1) !== side;
		link(shapes, chainEnd + CHAIN_LINK_LENGTH / 2 + fromEnd * CHAIN_PITCH, faceOn);
	}
	return mode === "motor_bottom" ? transform(shapes, ([x, y]) => [x, -y]) : shapes;
}

/** One link centred at `y`: its outside and hollow opening face-on, or a 7 mm bar edge-on. */
function link(shapes: Shapes, y: number, faceOn: boolean) {
	if (!faceOn) {
		solid(shapes, EDGE_ON_LINKS, [stadium(0, y, CHAIN_WIRE, CHAIN_LINK_LENGTH)]);
		return;
	}
	const outside = stadium(0, y, CHAIN_LINK_WIDTH, CHAIN_LINK_LENGTH);
	const opening = stadium(0, y, CHAIN_LINK_WIDTH - 2 * CHAIN_WIRE, CHAIN_LINK_LENGTH - 2 * CHAIN_WIRE);
	// Only the wire is solid: the opening stays hollow, so the next link shows through it.
	solid(shapes, FACE_ON_LINKS, [outside, opening], ring(outside, opening));
}

/** The hoist hanging from its hook at `top`; returns where its chain leaves the body. */
function hoist(shapes: Shapes, top: number, side: boolean): number {
	const width = side ? HOIST.depth : HOIST.width;
	const plateTop = top - HOOK_LENGTH;
	const bodyTop = plateTop - HOOK_PLATE.height;
	const bodyBottom = bodyTop - HOIST.height;
	// The plate sits on the body, which is in front where they meet.
	solid(shapes, HOIST_LAYER, [
		roundedRect(0, plateTop - HOOK_PLATE.height / 2, HOOK_PLATE.width, HOOK_PLATE.height, 6),
	]);
	solid(shapes, HOIST_LAYER, [roundedRect(0, (bodyTop + bodyBottom) / 2, width, HOIST.height, 24)]);
	// The gearbox joint and the motor's cooling fins read the body as a hoist, not a crate.
	const seam = bodyTop - HOIST.height * 0.38;
	line(shapes, HOIST_LAYER, [-width / 2, seam], [width / 2, seam]);
	for (let fin = 1; fin <= 4; fin++) {
		const y = seam - (HOIST.height * 0.62 * fin) / 5;
		line(shapes, HOIST_LAYER, [-width * 0.36, y], [width * 0.36, y]);
	}
	hook(shapes, plateTop, top, side);
	return bodyBottom;
}

/** A swivel hook: a shank up from the plate and a J opening to one side (edge-on from the side). */
function hook(shapes: Shapes, from: number, to: number, side: boolean) {
	const wire = 16;
	const length = to - from;
	if (side) {
		solid(shapes, HOIST_LAYER, [stadium(0, from + length / 2, wire, length)]);
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
	solid(shapes, HOIST_LAYER, [strokeOutline(path, wire)], [roundedRect(0, from + wire, wire, wire * 2, 2)]);
}

/** The fixing at the bottom of the object and the shackle on it; returns the bolt's height. */
function fixing(shapes: Shapes, bottom: number, side: boolean, anchor: CadChainAnchor): number {
	// Each fixing reaches up to where the shackle's bow wire passes through it.
	const bow =
		anchor === "steelflex"
			? steelflex(shapes, bottom)
			: anchor === "flange"
				? flange(shapes, bottom)
				: bottom + SHACKLE.wire / 2;
	const bolt = bow + SHACKLE.wire / 2 + SHACKLE.insideLength;
	shackle(shapes, bolt, side);
	return bolt;
}

/**
 * A steelflex round a truss chord: the sling wrapped the long way round under the chord and its two
 * legs rising 45° apart to meet in the shackle's bow. Returns where the legs meet.
 */
function steelflex(shapes: Shapes, bottom: number): number {
	const loop = CHORD_RADIUS + STEELFLEX_WIDTH / 2;
	const chord: PlanPoint = [0, bottom + CHORD_RADIUS + STEELFLEX_WIDTH];
	const lean = (STEELFLEX_LEG_DEGREES * Math.PI) / 180;
	// The legs lie side by side where they enter the bow: each leg's centreline ends half a sling
	// off the chain's line, measured square to the leg, so their inner edges just meet there rather
	// than crossing below it. Leaning ±lean, they leave the wrap where its radius is square to them,
	// which puts the bow this far above the chord's centre.
	const offset = STEELFLEX_WIDTH / 2 / Math.cos(lean);
	const meet = chord[1] + (loop - STEELFLEX_WIDTH / 2) / Math.sin(lean);
	const wrap: PlanPoint[] = [];
	const start = Math.PI - lean;
	const sweep = Math.PI + 2 * lean;
	for (let step = 0; step <= 24; step++) {
		const angle = start + (step / 24) * sweep;
		wrap.push([chord[0] + Math.cos(angle) * loop, chord[1] + Math.sin(angle) * loop]);
	}
	const legs: PlanPoint[][] = [
		[[-offset, meet], wrap[0]],
		wrap,
		[wrap[wrap.length - 1], [offset, meet]],
	];
	for (const path of legs) band(shapes, FIXING, path, STEELFLEX_WIDTH);
	// The chord comes last: the sling hugs it, so the chord's own edge draws where they touch.
	solid(shapes, FIXING, [circle(chord[0], chord[1], CHORD_RADIUS, 24)]);
	return meet;
}

/**
 * A flange on a pipe: a clamp of flat steel round the pipe and an eye plate standing up from it,
 * with the shackle's bow through the eye. Returns the eye's centre.
 */
function flange(shapes: Shapes, bottom: number): number {
	const pipe: PlanPoint = [0, bottom + PIPE_RADIUS + FLANGE.clamp];
	const clampOutside = PIPE_RADIUS + FLANGE.clamp;
	solid(shapes, FIXING, [circle(pipe[0], pipe[1], PIPE_RADIUS, 24)]);
	const plateTop = pipe[1] + clampOutside + FLANGE.plateLength;
	const eye = plateTop - FLANGE.eye - 6;
	// The plate is welded to the clamp's outside; the clamp, drawn over it, hides the joint.
	const plate = rectangle(-FLANGE.plateWidth / 2, pipe[1] + clampOutside - FLANGE.clamp / 2, FLANGE.plateWidth / 2, plateTop);
	solid(shapes, FIXING, [plate, circle(0, eye, FLANGE.eye, 16)], [plate]);
	const outer = circle(pipe[0], pipe[1], clampOutside, 24);
	const inner = circle(pipe[0], pipe[1], PIPE_RADIUS, 24);
	solid(shapes, FIXING, [outer, inner], ring(outer, inner));
	return eye;
}

/**
 * A bow shackle with its bolt at `bolt`. From the front: the U's two walls and bow face-on and the
 * bolt across their ends, a head on one side and a nut on the other. From the side: one wall edge-on
 * and the bolt's end in its nut.
 */
function shackle(shapes: Shapes, bolt: number, side: boolean) {
	const { wire, insideWidth, insideLength, bolt: boltWidth, overhang } = SHACKLE;
	const wallTop = bolt + boltWidth / 2 + 4;
	const bowBottom = bolt - insideLength - wire;
	if (side) {
		// Looking along the bolt, the near wall and the nut on the bolt's end are the nearest things
		// there: in front of the last link and of the link hanging through it.
		solid(shapes, SHACKLE_NEAR, [roundedRect(0, (wallTop + bowBottom) / 2, wire, wallTop - bowBottom, wire / 2)]);
		solid(shapes, SHACKLE_NEAR, [circle(0, bolt, 13, 6)]);
		solid(shapes, SHACKLE_NEAR, [circle(0, bolt, boltWidth / 2, 16)]);
		return;
	}
	const inner = insideWidth / 2;
	const outer = inner + wire;
	const bowCentre = bolt - insideLength + inner;
	const path = (radius: number): PlanPoint[] => {
		const points: PlanPoint[] = [[-radius, wallTop]];
		for (let step = 0; step <= 12; step++) {
			const angle = Math.PI + (step / 12) * Math.PI;
			points.push([Math.cos(angle) * radius, bowCentre + Math.sin(angle) * radius]);
		}
		points.push([radius, wallTop]);
		return points;
	};
	const outside = path(outer);
	const inside = path(inner);
	const pieces = outside.slice(1).map((point, index): PlanPoint[] => [outside[index], point, inside[index + 1], inside[index]]);
	solid(shapes, FIXING, [[...outside, ...[...inside].reverse()]], pieces);
	const end = outer + overhang;
	solid(shapes, BOLT, [rectangle(-end, bolt - boltWidth / 2, end, bolt + boltWidth / 2)]);
	solid(shapes, BOLT, [rectangle(-end - 8, bolt - 14, -end, bolt + 14)]);
	solid(shapes, BOLT, [rectangle(end - 2, bolt - 13, end + 8, bolt + 13)]);
}

/** A strip `width` wide along an open path, as its outline and the convex quads it covers. */
function band(shapes: Shapes, layer: number, path: PlanPoint[], width: number) {
	const half = width / 2;
	const left: PlanPoint[] = [];
	const right: PlanPoint[] = [];
	path.forEach((point, index) => {
		const before = path[Math.max(0, index - 1)];
		const after = path[Math.min(path.length - 1, index + 1)];
		const length = Math.hypot(after[0] - before[0], after[1] - before[1]) || 1;
		const normal: PlanPoint = [(-(after[1] - before[1]) / length) * half, ((after[0] - before[0]) / length) * half];
		left.push([point[0] + normal[0], point[1] + normal[1]]);
		right.push([point[0] - normal[0], point[1] - normal[1]]);
	});
	const pieces = left.slice(1).map((point, index): PlanPoint[] => [left[index], point, right[index + 1], right[index]]);
	solid(shapes, layer, [[...left, ...[...right].reverse()]], pieces);
}

function solid(shapes: Shapes, layer: number, edges: PlanPoint[][], area: PlanPoint[][] = edges) {
	shapes.parts.push({ layer, part: { kind: "solid", edges, area } });
}

function line(shapes: Shapes, layer: number, from: PlanPoint, to: PlanPoint) {
	shapes.parts.push({ layer, part: { kind: "line", line: { points: [from, to] } } });
}

/** The material between two loops whose points pair up one to one, as small convex quads. */
function ring(outside: PlanPoint[], inside: PlanPoint[]): PlanPoint[][] {
	return outside.map((point, index) => {
		const next = (index + 1) % outside.length;
		return [point, outside[next], inside[next], inside[index]];
	});
}

function finish(shapes: Shapes): PlanGeometry {
	// A stable sort keeps drawing order within a layer.
	const parts = [...shapes.parts]
		.sort((left, right) => left.layer - right.layer)
		.map(({ part }) => part);
	const triangles: PlanTriangle[] = [];
	for (const part of parts) {
		if (part.kind !== "solid") continue;
		for (const mask of part.area)
			for (let index = 1; index < mask.length - 1; index++)
				triangles.push({ points: [mask[0], mask[index], mask[index + 1]], color: FILL });
	}
	return { source: "typed", triangles, ...hideCoveredEdges(parts) };
}

function transform(shapes: Shapes, map: (point: PlanPoint) => PlanPoint): Shapes {
	return {
		parts: shapes.parts.map(({ layer, part }) => ({
			layer,
			part:
				part.kind === "solid"
					? {
							kind: "solid",
							edges: part.edges.map((edge) => edge.map(map)),
							area: part.area.map((area) => area.map(map)),
						}
					: {
							kind: "line",
							line: { ...part.line, points: part.line.points.map(map) as PlanLine["points"] },
						},
		})),
	};
}

/** A rounded rectangle with a 50% corner radius: straight sides along its longer axis. */
export function stadium(x: number, y: number, width: number, height: number): PlanPoint[] {
	return roundedRect(x, y, width, height, Math.min(width, height) / 2);
}

function rectangle(left: number, bottom: number, right: number, top: number): PlanPoint[] {
	return [
		[left, bottom],
		[right, bottom],
		[right, top],
		[left, top],
	];
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
