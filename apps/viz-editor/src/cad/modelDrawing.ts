/**
 * The shipped model drawings (`assets/models/2d`) a fixture without its own model is drawn from.
 *
 * A drawing is plain SVG in the model's millimetres with the page's y running down: a `silhouette`
 * group holding even-odd filled paths (a loop inside a loop is a hole), and a `lines` group whose
 * `base`, `yoke` and `head` groups hold the visible edges as polylines. The silhouette becomes the
 * depth and pick mask, the polylines the linework.
 *
 * A hinged lamp's drawing also keeps its hanging hardware apart: a `hardware` group of lines, a
 * `silhouette-hardware` path beside the `silhouette-body` one, the hinge on the page in
 * `data-hinge="x y"` and, in `data-bracket`, the bracket angle the drawn body's pose equals (absent
 * is 0, hanging as modelled). That lets a side view turn the body about the hinge to the pose the
 * Visualizer gives the fixture's bracket angle while the hardware stays where it hangs, laid over
 * the body. A drawing without those parts is drawn as it is.
 *
 * The drawings are edited by hand, so this reads what a vector editor writes, the same way the
 * Visualizer's reader (`crates/viz/project/src/plan_drawing/svg.rs`) does: curves and arcs are
 * followed as short straight runs, `rect`, `circle`, `ellipse`, `line`, `polyline` and `polygon`
 * are read as outlines, and `transform` attributes on groups and shapes are applied.
 */
import { ShapeUtils, Vector2 } from "three";
import { hideCoveredEdges } from "./hiddenLines";
import type {
	PlanGeometry,
	PlanLine,
	PlanPoint,
	PlanTriangle,
} from "./projection";
import type { CadDrawing, CadModelDrawingView, CadViewDirection } from "./types";

const DARK: [number, number, number] = [0.13, 0.15, 0.18];

/** How many straight runs follow one curve; a circle or ellipse is three curves' worth. */
const CURVE_STEPS = 12;

type Triangle = [PlanPoint, PlanPoint, PlanPoint];
type Segment = [PlanPoint, PlanPoint];
/** An affine transform `[a, b, c, d, e, f]`, mapping `(x, y)` to `(a x + c y + e, b x + d y + f)`. */
type Matrix = [number, number, number, number, number, number];

const IDENTITY: Matrix = [1, 0, 0, 1, 0, 0];
const NUMBER = /[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g;
const PATH_TOKEN = /[A-DF-Za-df-z]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g;

export interface DrawingPiece {
	triangles: Triangle[];
	lines: Segment[];
}

export interface ParsedModelDrawing {
	/** The whole silhouette's area, in plan millimetres with y up. */
	triangles: Triangle[];
	/** Every visible edge, one segment each. */
	lines: Segment[];
	/** The lamp itself: everything that is not hanging hardware. */
	body: DrawingPiece;
	/** The hanging frame and coupler, when the drawing keeps them apart. */
	hardware: DrawingPiece;
	/** Where the body turns in its frame, in plan millimetres, when the drawing records it. */
	hinge?: PlanPoint;
	/** The bracket angle the drawn body's pose equals, in degrees, when the drawing records it. */
	bracket?: number;
}

/** One outline in page millimetres, y down. */
interface Subpath {
	points: PlanPoint[];
	closed: boolean;
}

/** Which of a model's three drawings a CAD view shows, and whether it is seen mirrored. */
export function modelDrawingViewForCad(view: CadViewDirection): {
	view: CadModelDrawingView["view"];
	mirrored: boolean;
} {
	switch (view) {
		case "top_down":
			return { view: "top", mirrored: false };
		case "front_to_back":
			return { view: "front", mirrored: false };
		case "back_to_front":
			return { view: "front", mirrored: true };
		case "left_to_right":
			return { view: "side", mirrored: false };
		case "right_to_left":
			return { view: "side", mirrored: true };
	}
}

/** The elevation views in the order a quarter turn of yaw (desk z) steps a lamp through them. */
const ELEVATION_RING: readonly CadViewDirection[] = [
	"front_to_back",
	"left_to_right",
	"back_to_front",
	"right_to_left",
];

/**
 * The view a CAD elevation shows of a lamp yawed `yawDegrees` (its desk z rotation), to the
 * nearest quarter turn: a lamp turned 90° shows its side to the front elevation, so its bracket
 * turn is seen there. The top view is unchanged; it turns the drawing by the yaw itself.
 */
export function lampRelativeView(
	view: CadViewDirection,
	yawDegrees = 0,
): CadViewDirection {
	const index = ELEVATION_RING.indexOf(view);
	if (index < 0 || !Number.isFinite(yawDegrees)) return view;
	const quarters = Math.round(yawDegrees / 90);
	return ELEVATION_RING[(((index + quarters) % 4) + 4) % 4];
}

/**
 * Read a drawing's silhouette into triangles and its linework into segments. Everything under a
 * `silhouette…` group or with a `silhouette…` id is area, everything under `lines` is linework, and
 * `origin` is ignored; `silhouette-hardware` and `hardware` are the hanging hardware.
 */
export function parseModelDrawing(svg: string): ParsedModelDrawing {
	const bodyRings: PlanPoint[][] = [];
	const hardwareRings: PlanPoint[][] = [];
	const body: DrawingPiece = { triangles: [], lines: [] };
	const hardware: DrawingPiece = { triangles: [], lines: [] };
	const groups: { id: string; matrix: Matrix }[] = [];
	for (const match of svg.matchAll(
		/<!--[\s\S]*?-->|<(\/?)([A-Za-z][\w:.-]*)([^>]*?)(\/?)>/g,
	)) {
		const [, closing, name, attributes = "", selfClosing] = match;
		if (!name) continue;
		if (closing) {
			if (name === "g") groups.pop();
			continue;
		}
		const parent = groups.at(-1)?.matrix ?? IDENTITY;
		const transform = attribute(attributes, "transform");
		const matrix = transform
			? multiply(parent, parseTransform(transform))
			: parent;
		if (name === "g") {
			if (!selfClosing)
				groups.push({ id: attribute(attributes, "id") ?? "", matrix });
			continue;
		}
		const ids = groups.map((group) => group.id);
		if (ids.includes("origin")) continue;
		const subpaths = shape(name, attributes);
		if (!subpaths.length) continue;
		const ownId = attribute(attributes, "id") ?? "";
		const silhouette =
			ownId.startsWith("silhouette") ||
			ids.some((id) => id.startsWith("silhouette"));
		const linework = ids.includes("lines");
		const isHardware =
			ownId === "silhouette-hardware" ||
			ids.includes("silhouette-hardware") ||
			ids.includes("hardware");
		for (const { points, closed } of subpaths) {
			if (points.length < 2) continue;
			const placed = points.map((point): PlanPoint => {
				const [x, y] = apply(matrix, point);
				return [x, -y];
			});
			if (silhouette) {
				if (placed.length >= 3)
					(isHardware ? hardwareRings : bodyRings).push(placed);
			} else if (linework) {
				const piece = isHardware ? hardware : body;
				for (let index = 1; index < placed.length; index++)
					piece.lines.push([placed[index - 1], placed[index]]);
				if (closed && placed.length > 2)
					piece.lines.push([placed[placed.length - 1], placed[0]]);
			}
		}
	}
	body.triangles = triangulateRings(bodyRings);
	hardware.triangles = triangulateRings(hardwareRings);
	const root = svg.match(/<svg\b([^>]*)>/)?.[1] ?? "";
	const hinge = attribute(root, "data-hinge")?.match(NUMBER)?.map(Number);
	const bracket = Number(attribute(root, "data-bracket") ?? Number.NaN);
	return {
		triangles: [...body.triangles, ...hardware.triangles],
		lines: [...body.lines, ...hardware.lines],
		body,
		hardware,
		hinge:
			hinge?.length === 2 && hinge.every(Number.isFinite)
				? [hinge[0], -hinge[1]]
				: undefined,
		bracket: Number.isFinite(bracket) ? bracket : undefined,
	};
}

/**
 * How far a side view turns a drawing's body about its hinge, in degrees about the lamp's
 * transverse axis in the Visualizer's bracket sense: from the pose it was drawn at (`data-bracket`,
 * 0 when absent) to the fixture's configured bracket angle, 0 included, so the body sits exactly as
 * the 3D view poses it. Only a side view of a drawing that records a hinge and separate hardware
 * turns; top and front views keep their drawn poses.
 */
export function bodyTurnDegrees(
	parsed: ParsedModelDrawing,
	view: CadViewDirection,
	bracketAngle = 0,
): number {
	if (view !== "left_to_right" && view !== "right_to_left") return 0;
	const separate =
		parsed.hardware.triangles.length > 0 || parsed.hardware.lines.length > 0;
	if (!parsed.hinge || !separate) return 0;
	return bracketAngle - (parsed.bracket ?? 0);
}

const parsedDrawings = new Map<string, ParsedModelDrawing>();
const arrangedDrawings = new Map<string, DrawingPiece>();

function cachedParse(svg: string): ParsedModelDrawing {
	let parsed = parsedDrawings.get(svg);
	if (!parsed) {
		if (parsedDrawings.size > 256) parsedDrawings.clear();
		parsed = parseModelDrawing(svg);
		parsedDrawings.set(svg, parsed);
	}
	return parsed;
}

/**
 * The drawing with its body turned `turn` degrees nose-down about the hinge and the hardware laid
 * over it: body lines under the hardware's silhouette are hidden, since the frame hangs in front.
 */
function arranged(
	svg: string,
	parsed: ParsedModelDrawing,
	turn: number,
): DrawingPiece {
	const separate =
		parsed.hardware.triangles.length > 0 || parsed.hardware.lines.length > 0;
	if (!separate) return parsed;
	const key = `${turn} ${svg}`;
	const cached = arrangedDrawings.get(key);
	if (cached) return cached;
	const [hx, hy] = parsed.hinge ?? [0, 0];
	// Plan y runs up, so nose-down (clockwise on the page) is a negative plan angle.
	const cos = Math.cos((turn * Math.PI) / 180);
	const sin = Math.sin((turn * Math.PI) / 180);
	const rotate = (point: PlanPoint): PlanPoint =>
		turn === 0
			? point
			: [
					hx + cos * (point[0] - hx) + sin * (point[1] - hy),
					hy - sin * (point[0] - hx) + cos * (point[1] - hy),
				];
	const bodyLines = parsed.body.lines.map(
		(line) => line.map(rotate) as Segment,
	);
	const visible = parsed.hardware.triangles.length
		? hideCoveredEdges([
				...bodyLines.map((points) => ({
					kind: "line" as const,
					line: { points },
				})),
				{ kind: "solid", edges: [], area: parsed.hardware.triangles },
			]).lines.map((line) => line.points)
		: bodyLines;
	const result: DrawingPiece = {
		triangles: [
			...parsed.body.triangles.map(
				(triangle) => triangle.map(rotate) as Triangle,
			),
			...parsed.hardware.triangles,
		],
		lines: [...visible, ...parsed.hardware.lines],
	};
	if (arrangedDrawings.size > 256) arrangedDrawings.clear();
	arrangedDrawings.set(key, result);
	return result;
}

/**
 * A fixture's shipped model drawing for one CAD view, at the fixture's size, or `null` when its
 * profile is not drawn from a shipped model. Without mounting hardware the `-no-clamp` drawing is
 * used wherever the model has one. An elevation shows the drawing of the side the lamp's yaw turns
 * toward it, and wherever that is the lamp's side, a hinged body is turned by `bracketAngle` while
 * its hanging hardware stays put.
 */
export function modelDrawingGeometry(
	drawing: CadDrawing | undefined,
	view: CadViewDirection,
	mountingHardware = true,
	bracketAngle = 0,
	yawDegrees = 0,
): PlanGeometry | null {
	const model = drawing?.modelDrawing;
	if (!model) return null;
	const seen = lampRelativeView(view, yawDegrees);
	const { view: name, mirrored } = modelDrawingViewForCad(seen);
	const entry = model.views.find((candidate) => candidate.view === name);
	if (!entry) return null;
	const svg =
		!mountingHardware && entry.noClampSvg ? entry.noClampSvg : entry.svg;
	const parsed = cachedParse(svg);
	if (!parsed.triangles.length && !parsed.lines.length) return null;
	const posed = arranged(
		svg,
		parsed,
		bodyTurnDegrees(parsed, seen, bracketAngle),
	);
	const scale = model.scale > 0 ? model.scale : 1;
	const place = (point: PlanPoint): PlanPoint => [
		point[0] * scale * (mirrored ? -1 : 1),
		point[1] * scale,
	];
	return {
		source: "model_drawing",
		triangles: posed.triangles.map(
			(points): PlanTriangle => ({
				points: points.map(place) as PlanTriangle["points"],
				color: DARK,
			}),
		),
		outlines: [],
		lines: posed.lines.map(
			(points): PlanLine => ({
				points: points.map(place) as PlanLine["points"],
			}),
		),
	};
}

/** The outline of one SVG shape element, in its own page coordinates. */
function shape(name: string, attributes: string): Subpath[] {
	const number = (key: string) => {
		const first = attribute(attributes, key)?.match(NUMBER)?.[0];
		return first === undefined ? undefined : Number(first);
	};
	switch (name) {
		case "path": {
			const data = attribute(attributes, "d");
			return data ? parsePathData(data) : [];
		}
		case "polyline":
		case "polygon": {
			const values = (attribute(attributes, "points") ?? "")
				.match(NUMBER)
				?.map(Number);
			const points: PlanPoint[] = [];
			for (let index = 0; index + 1 < (values?.length ?? 0); index += 2)
				points.push([values![index], values![index + 1]]);
			return points.length ? [{ points, closed: name === "polygon" }] : [];
		}
		case "line":
			return [
				{
					points: [
						[number("x1") ?? 0, number("y1") ?? 0],
						[number("x2") ?? 0, number("y2") ?? 0],
					],
					closed: false,
				},
			];
		case "rect": {
			const [x, y] = [number("x") ?? 0, number("y") ?? 0];
			const [width, height] = [number("width"), number("height")];
			if (width === undefined || height === undefined) return [];
			return [
				{
					points: [
						[x, y],
						[x + width, y],
						[x + width, y + height],
						[x, y + height],
					],
					closed: true,
				},
			];
		}
		case "circle":
		case "ellipse": {
			const [cx, cy] = [number("cx") ?? 0, number("cy") ?? 0];
			const [rx, ry] =
				name === "circle"
					? [number("r") ?? 0, number("r") ?? 0]
					: [number("rx") ?? 0, number("ry") ?? 0];
			if (rx <= 0 || ry <= 0) return [];
			const steps = CURVE_STEPS * 3;
			const points: PlanPoint[] = [];
			for (let step = 0; step < steps; step++) {
				const angle = (step / steps) * Math.PI * 2;
				points.push([cx + rx * Math.cos(angle), cy + ry * Math.sin(angle)]);
			}
			return [{ points, closed: true }];
		}
		default:
			return [];
	}
}

/** `translate`, `scale`, `rotate` (about an optional centre), `skewX`, `skewY` and `matrix`. */
function parseTransform(text: string): Matrix {
	let matrix = IDENTITY;
	for (const [, name, body] of text.matchAll(/([A-Za-z]+)\s*\(([^)]*)\)/g)) {
		const values = body.match(NUMBER)?.map(Number) ?? [];
		const value = (index: number, fallback: number) =>
			values[index] ?? fallback;
		let next: Matrix = IDENTITY;
		switch (name) {
			case "translate":
				next = [1, 0, 0, 1, value(0, 0), value(1, 0)];
				break;
			case "scale":
				next = [value(0, 1), 0, 0, value(1, value(0, 1)), 0, 0];
				break;
			case "rotate": {
				const radians = (value(0, 0) * Math.PI) / 180;
				const [cx, cy] = [value(1, 0), value(2, 0)];
				const turn: Matrix = [
					Math.cos(radians),
					Math.sin(radians),
					-Math.sin(radians),
					Math.cos(radians),
					0,
					0,
				];
				next = multiply(
					[1, 0, 0, 1, cx, cy],
					multiply(turn, [1, 0, 0, 1, -cx, -cy]),
				);
				break;
			}
			case "skewX":
				next = [1, 0, Math.tan((value(0, 0) * Math.PI) / 180), 1, 0, 0];
				break;
			case "skewY":
				next = [1, Math.tan((value(0, 0) * Math.PI) / 180), 0, 1, 0, 0];
				break;
			case "matrix":
				if (values.length >= 6) next = values.slice(0, 6) as Matrix;
				break;
		}
		matrix = multiply(matrix, next);
	}
	return matrix;
}

/** `outer` after `inner`: a point is transformed by `inner` first. */
function multiply(outer: Matrix, inner: Matrix): Matrix {
	const [a, b, c, d, e, f] = outer;
	const [g, h, i, j, k, l] = inner;
	return [
		a * g + c * h,
		b * g + d * h,
		a * i + c * j,
		b * i + d * j,
		a * k + c * l + e,
		b * k + d * l + f,
	];
}

function apply([a, b, c, d, e, f]: Matrix, [x, y]: PlanPoint): PlanPoint {
	return [a * x + c * y + e, b * x + d * y + f];
}

/** Every path command in either case; curves and arcs are followed as short straight runs. */
function parsePathData(data: string): Subpath[] {
	const tokens = data.match(PATH_TOKEN) ?? [];
	const isCommand = (token: string) => /^[A-Za-z]$/.test(token);
	const subpaths: Subpath[] = [];
	let current: Subpath | null = null;
	let command = "M";
	let at: PlanPoint = [0, 0];
	let start: PlanPoint = [0, 0];
	// The last control point, for the smooth curves that reflect it.
	type Control = { kind: "c" | "q"; point: PlanPoint };
	let control = null as Control | null;
	let index = 0;
	while (index < tokens.length) {
		if (isCommand(tokens[index])) {
			command = tokens[index];
			index += 1;
			if (command === "z" || command === "Z") {
				if (current) current.closed = true;
				current = null;
				at = start;
				control = null;
			}
			continue;
		}
		const lower = command.toLowerCase();
		const arity =
			lower === "h" || lower === "v"
				? 1
				: lower === "s" || lower === "q"
					? 4
					: lower === "c"
						? 6
						: lower === "a"
							? 7
							: lower === "z"
								? 0
								: 2;
		if (arity === 0) {
			index += 1;
			continue;
		}
		const args: number[] = [];
		while (
			args.length < arity &&
			index + args.length < tokens.length &&
			!isCommand(tokens[index + args.length])
		)
			args.push(Number(tokens[index + args.length]));
		if (args.length < arity) {
			index += Math.max(args.length, 1);
			continue;
		}
		index += arity;
		const relative = command === lower;
		const from = at;
		const point = (x: number, y: number): PlanPoint =>
			relative ? [from[0] + x, from[1] + y] : [x, y];
		if (lower === "m") {
			const end = point(args[0], args[1]);
			current = { points: [end], closed: false };
			subpaths.push(current);
			start = end;
			at = end;
			control = null;
			// Coordinates after a move continue as lines.
			command = relative ? "l" : "L";
			continue;
		}
		if (!current) {
			current = { points: [], closed: false };
			subpaths.push(current);
		}
		const points = current.points;
		let end: PlanPoint;
		let next = null as Control | null;
		if (lower === "h") {
			end = [relative ? from[0] + args[0] : args[0], from[1]];
			points.push(end);
		} else if (lower === "v") {
			end = [from[0], relative ? from[1] + args[0] : args[0]];
			points.push(end);
		} else if (lower === "c" || lower === "s") {
			let first: PlanPoint;
			let second: PlanPoint;
			if (lower === "c") {
				first = point(args[0], args[1]);
				second = point(args[2], args[3]);
				end = point(args[4], args[5]);
			} else {
				first =
					control?.kind === "c"
						? [2 * from[0] - control.point[0], 2 * from[1] - control.point[1]]
						: from;
				second = point(args[0], args[1]);
				end = point(args[2], args[3]);
			}
			for (let step = 1; step <= CURVE_STEPS; step++) {
				const t = step / CURVE_STEPS;
				const u = 1 - t;
				const w = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
				points.push([
					w[0] * from[0] + w[1] * first[0] + w[2] * second[0] + w[3] * end[0],
					w[0] * from[1] + w[1] * first[1] + w[2] * second[1] + w[3] * end[1],
				]);
			}
			next = { kind: "c", point: second };
		} else if (lower === "q" || lower === "t") {
			let handle: PlanPoint;
			if (lower === "q") {
				handle = point(args[0], args[1]);
				end = point(args[2], args[3]);
			} else {
				handle =
					control?.kind === "q"
						? [2 * from[0] - control.point[0], 2 * from[1] - control.point[1]]
						: from;
				end = point(args[0], args[1]);
			}
			for (let step = 1; step <= CURVE_STEPS; step++) {
				const t = step / CURVE_STEPS;
				const u = 1 - t;
				points.push([
					u * u * from[0] + 2 * u * t * handle[0] + t * t * end[0],
					u * u * from[1] + 2 * u * t * handle[1] + t * t * end[1],
				]);
			}
			next = { kind: "q", point: handle };
		} else if (lower === "a") {
			end = point(args[5], args[6]);
			points.push(
				...arc(from, end, args[0], args[1], args[2], args[3] !== 0, args[4] !== 0),
			);
		} else {
			end = point(args[0], args[1]);
			points.push(end);
		}
		at = end;
		control = next;
	}
	return subpaths;
}

/**
 * The points along an elliptical arc from `from` to `to`, excluding `from`: SVG's endpoint form
 * converted to centre form.
 */
function arc(
	from: PlanPoint,
	to: PlanPoint,
	radiusX: number,
	radiusY: number,
	rotation: number,
	large: boolean,
	sweep: boolean,
): PlanPoint[] {
	let rx = Math.abs(radiusX);
	let ry = Math.abs(radiusY);
	if (
		rx < 1e-6 ||
		ry < 1e-6 ||
		Math.abs(from[0] - to[0]) + Math.abs(from[1] - to[1]) < 1e-6
	)
		return [to];
	const sin = Math.sin((rotation * Math.PI) / 180);
	const cos = Math.cos((rotation * Math.PI) / 180);
	const dx = (from[0] - to[0]) / 2;
	const dy = (from[1] - to[1]) / 2;
	const x1 = cos * dx + sin * dy;
	const y1 = -sin * dx + cos * dy;
	const scale = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
	if (scale > 1) {
		rx *= Math.sqrt(scale);
		ry *= Math.sqrt(scale);
	}
	const numerator = Math.max(
		0,
		rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1,
	);
	const denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
	let factor = Math.sqrt(numerator / Math.max(denominator, 1e-12));
	if (large === sweep) factor = -factor;
	const cx1 = (factor * rx * y1) / ry;
	const cy1 = (-factor * ry * x1) / rx;
	const centre: PlanPoint = [
		cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2,
		sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2,
	];
	const start = Math.atan2((y1 - cy1) / ry, (x1 - cx1) / rx);
	let delta = Math.atan2((-y1 - cy1) / ry, (-x1 - cx1) / rx) - start;
	const tau = Math.PI * 2;
	if (sweep && delta < 0) delta += tau;
	else if (!sweep && delta > 0) delta -= tau;
	const steps = Math.max(1, Math.ceil((Math.abs(delta) / tau) * CURVE_STEPS * 4));
	const points: PlanPoint[] = [];
	for (let step = 1; step <= steps; step++) {
		if (step === steps) {
			points.push(to);
			break;
		}
		const theta = start + (delta * step) / steps;
		const [x, y] = [rx * Math.cos(theta), ry * Math.sin(theta)];
		points.push([cos * x - sin * y + centre[0], sin * x + cos * y + centre[1]]);
	}
	return points;
}

/** Even-odd rings into triangles: each ring at an even nesting depth is a solid with its holes. */
function triangulateRings(input: PlanPoint[][]): Triangle[] {
	const rings = input
		.map((ring) => {
			const points = ring.filter(
				(point, index) =>
					index === 0 ||
					point[0] !== ring[index - 1][0] ||
					point[1] !== ring[index - 1][1],
			);
			const first = points[0];
			const last = points[points.length - 1];
			if (points.length > 1 && first[0] === last[0] && first[1] === last[1])
				points.pop();
			return points;
		})
		.filter((ring) => ring.length >= 3 && Math.abs(ringArea(ring)) > 1e-6);
	const containers = rings.map((ring, index) =>
		rings
			.map((_, other) => other)
			.filter(
				(other) =>
					other !== index &&
					Math.abs(ringArea(rings[other])) > Math.abs(ringArea(ring)) &&
					pointInRing(ring[0], rings[other]),
			),
	);
	const triangles: Triangle[] = [];
	rings.forEach((ring, index) => {
		const depth = containers[index].length;
		if (depth % 2 !== 0) return;
		const holes = rings.filter(
			(_, other) =>
				containers[other].length === depth + 1 &&
				containers[other].includes(index),
		);
		const vertices = [ring, ...holes].flat();
		// Rings arrive without a repeated closing point, so three's triangulation removes none and
		// its indices address `vertices` directly.
		const faces = ShapeUtils.triangulateShape(
			ring.map(([x, y]) => new Vector2(x, y)),
			holes.map((hole) => hole.map(([x, y]) => new Vector2(x, y))),
		);
		for (const [first, second, third] of faces)
			triangles.push([vertices[first], vertices[second], vertices[third]]);
	});
	return triangles;
}

function ringArea(ring: readonly PlanPoint[]): number {
	let area = 0;
	for (let index = 0; index < ring.length; index++) {
		const [x1, y1] = ring[index];
		const [x2, y2] = ring[(index + 1) % ring.length];
		area += x1 * y2 - x2 * y1;
	}
	return area / 2;
}

function pointInRing(point: PlanPoint, ring: readonly PlanPoint[]): boolean {
	let inside = false;
	for (
		let current = 0, previous = ring.length - 1;
		current < ring.length;
		previous = current++
	) {
		const [cx, cy] = ring[current];
		const [px, py] = ring[previous];
		if (
			cy > point[1] !== py > point[1] &&
			point[0] < ((px - cx) * (point[1] - cy)) / (py - cy) + cx
		)
			inside = !inside;
	}
	return inside;
}

/** An attribute's value in either quote style, or `null`. */
function attribute(source: string, name: string): string | null {
	const match = source.match(
		new RegExp(`(?:^|\\s)${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`),
	);
	return match ? (match[1] ?? match[2]) : null;
}
