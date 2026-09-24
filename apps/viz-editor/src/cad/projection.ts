import { chainPlan } from "./chainPlan";
import { crowdPlan, seedOf } from "./crowdPlan";
import { curtainPlan } from "./curtainPlan";
import { discoBallPlan, flightRackPlan, lineArrayPlan, paSpeakerPlan } from "./equipmentPlan";
import { hideCoveredEdges } from "./hiddenLines";
import {
	bakedYawQuarterTurns,
	lampRelativeView,
	modelDrawingGeometry,
} from "./modelDrawing";
import { stairElevation, stairPlan } from "./stairPlan";
import { trussPlan } from "./trussPlan";
import type {
	CadDrawing,
	CadEntity,
	CadProjectionView,
	CadStairHandrails,
	CadViewDirection,
} from "./types";

/**
 * How an audience is drawn lives in `crowdPlan`, but the crowd shares its stature and its seed
 * with the 3D view and the tests, which have always asked the plan symbols for them. They keep
 * reading them from here so moving the drawing out changed nothing a caller can see.
 */
export {
	audiencePersonHeight,
	audiencePersonScale,
	crowdGrid,
	seedOf,
} from "./crowdPlan";

export type PlanPoint = [number, number];

export interface PlanTriangle {
	points: [PlanPoint, PlanPoint, PlanPoint];
	color: [number, number, number];
	depths?: [number, number, number];
}

export interface PlanLine {
	points: [PlanPoint, PlanPoint];
	depths?: [number, number];
}

export interface PlanGeometry {
	source: "live_model" | "model_drawing" | "model" | "typed" | "unknown";
	triangles: PlanTriangle[];
	outlines: PlanPoint[][];
	lines: PlanLine[];
	/**
	 * How much of the entity's yaw this geometry already shows, in quarter turns, because reading
	 * the drawing of another side answered it. Whatever turns the geometry on the page afterwards
	 * has that much less to turn; see `planBasis`.
	 */
	yawQuarterTurnsShown?: number;
}

interface Polygon {
	points: PlanPoint[];
	color: [number, number, number];
}

const BASE: [number, number, number] = [0.25, 0.28, 0.32];
const BODY: [number, number, number] = [0.38, 0.42, 0.47];
const DETAIL: [number, number, number] = [0.57, 0.61, 0.66];
const DARK: [number, number, number] = [0.13, 0.15, 0.18];

export function projectionViewForCad(
	view: CadViewDirection,
): CadProjectionView {
	switch (view) {
		case "top_down":
			return "top";
		case "left_to_right":
			return "left";
		case "right_to_left":
			return "right";
		case "front_to_back":
			return "front";
		case "back_to_front":
			return "back";
	}
}

export interface PlanGeometryOptions {
	/** Draw clamps and other mounting hardware; a print page can leave them off. Default true. */
	mountingHardware?: boolean;
}

export function entityPlanGeometry(
	entity: CadEntity,
	drawing: CadDrawing | undefined,
	view: CadViewDirection,
	options: PlanGeometryOptions = {},
): PlanGeometry {
	const geometry = orientedPlanGeometry(
		entity,
		drawing,
		view,
		options.mountingHardware !== false,
	);
	// Typed symbols are authored with a fixture's front toward +y and are mirrored so the front faces
	// downstage (plan −y, desk y running upstage). A model drawing or projection SVG from above has
	// the model's +Z (downstage, as in the Visualizer) down the page, which reading its page y
	// upward already puts at plan −y; a live model is projected from model coordinates directly.
	return view === "top_down" &&
		(geometry.source === "typed" || geometry.source === "unknown")
		? mirrorVertically(geometry)
		: geometry;
}

function mirrorVertically(geometry: PlanGeometry): PlanGeometry {
	const flip = (point: PlanPoint): PlanPoint => [point[0], -point[1]];
	return {
		...geometry,
		triangles: geometry.triangles.map((triangle) => ({
			...triangle,
			points: triangle.points.map(flip) as PlanTriangle["points"],
		})),
		outlines: geometry.outlines.map((outline) => outline.map(flip)),
		lines: geometry.lines.map((line) => ({
			...line,
			points: line.points.map(flip) as PlanLine["points"],
		})),
	};
}

function orientedPlanGeometry(
	entity: CadEntity,
	drawing: CadDrawing | undefined,
	view: CadViewDirection,
	mountingHardware: boolean,
): PlanGeometry {
	const type = entityType(entity);
	// Crowd-area models describe a procedural volume and read as an unexplained block in plan.
	// Modeled fixtures and venue objects—including trusses—keep their canonical generated SVG.
	if (isSemanticPlanSymbol(type)) return typedGeometry(entity, view, type);
	const live = liveModelGeometry(entity, drawing, view);
	if (live) return live;
	// A lamp without a model of its own is drawn from the shipped model the Visualizer shows;
	// generated scenery is built, not drawn from a body, and keeps its own drawing.
	const modelDrawing = entity.scenery
		? null
		: modelDrawingGeometry(
				drawing,
				view,
				mountingHardware,
				entity.bracketAngle,
				entity.rotationDegrees[2],
			);
	if (modelDrawing) return modelDrawing;
	// A projection sheet keeps one drawing per side, so an elevation reads the side the lamp's yaw
	// turns toward it, exactly as a model drawing does. Generated scenery is a volume rather than a
	// body with sides, and keeps the sheet of the view asked for.
	const quarters = entity.scenery
		? 0
		: bakedYawQuarterTurns(view, entity.rotationDegrees[2]);
	const seen = entity.scenery
		? view
		: lampRelativeView(view, entity.rotationDegrees[2]);
	const projection = drawing?.projections.find(
		(candidate) => candidate.view === projectionViewForCad(seen),
	);
	if (projection) {
		const parsed = parseProjection(
			projection.svg,
			projection.originMillimetres,
		);
		if (parsed.triangles.length)
			return { ...parsed, yawQuarterTurnsShown: quarters };
	}
	return typedGeometry(entity, view, type);
}

function liveModelGeometry(
	entity: CadEntity,
	drawing: CadDrawing | undefined,
	view: CadViewDirection,
): PlanGeometry | null {
	const pose = view === "top_down" ? "top" : "elevation";
	const mesh = drawing?.liveMeshes?.find(
		(candidate) => candidate.pose === pose,
	);
	if (!mesh?.triangles.length) return null;
	const faces = mesh.triangles
		.map((triangle, index) => {
			const world = triangle.pointsMillimetres.map((point) =>
				rotateModelPoint(
					bracketTurnedPoint(point, entity.bracketAngle ?? 0),
					entity.rotationDegrees,
				),
			) as [
				[number, number, number],
				[number, number, number],
				[number, number, number],
			];
			const points = world.map((point) => projectModelPoint(point, view)) as [
				PlanPoint,
				PlanPoint,
				PlanPoint,
			];
			const area = Math.abs(
				(points[1][0] - points[0][0]) * (points[2][1] - points[0][1]) -
					(points[1][1] - points[0][1]) * (points[2][0] - points[0][0]),
			);
			return {
				index,
				world,
				points,
				depths: world.map((point) => modelDepth(point, view)) as [
					number,
					number,
					number,
				],
				depth:
					world.reduce((sum, point) => sum + modelDepth(point, view), 0) / 3,
				area,
			};
		})
		.filter((face) => face.area >= 0.08)
		.sort(
			(left, right) => right.depth - left.depth || left.index - right.index,
		);
	const triangles = faces.map((face) => ({
		points: face.points,
		color: DARK,
		depths: face.depths,
	}));
	return triangles.length
		? {
				source: "live_model",
				triangles,
				outlines: [],
				lines: modelFeatureLines(faces, view),
			}
		: null;
}

function modelFeatureLines(
	faces: readonly {
		world: [
			[number, number, number],
			[number, number, number],
			[number, number, number],
		];
		points: [PlanPoint, PlanPoint, PlanPoint];
		depths: [number, number, number];
	}[],
	view: CadViewDirection,
): PlanLine[] {
	const camera = cameraVector(view);
	type Edge = {
		points: [PlanPoint, PlanPoint];
		depths: [number, number];
		faces: { normal: [number, number, number]; front: boolean }[];
	};
	const edges = new Map<string, Edge>();
	for (const face of faces) {
		const normal = normalOf(face.world);
		const front = dot3(normal, camera) > 0.0001;
		for (const [first, second] of [
			[0, 1],
			[1, 2],
			[2, 0],
		] as const) {
			const firstKey = point3Key(face.world[first]);
			const secondKey = point3Key(face.world[second]);
			const forward = firstKey < secondKey;
			const key = forward
				? `${firstKey}|${secondKey}`
				: `${secondKey}|${firstKey}`;
			const edge = edges.get(key);
			if (edge) {
				edge.faces.push({ normal, front });
			} else {
				edges.set(key, {
					points: forward
						? [face.points[first], face.points[second]]
						: [face.points[second], face.points[first]],
					depths: forward
						? [face.depths[first], face.depths[second]]
						: [face.depths[second], face.depths[first]],
					faces: [{ normal, front }],
				});
			}
		}
	}
	const visible = [...edges.values()].filter((edge) => {
		const front = edge.faces.filter((face) => face.front);
		if (!front.length) return false;
		if (edge.faces.length === 1 || front.length !== edge.faces.length)
			return true;
		return front.some((face, index) =>
			front
				.slice(index + 1)
				.some((other) => dot3(face.normal, other.normal) < 0.82),
		);
	});
	if (visible.length)
		return visible.map(({ points, depths }) => ({ points, depths }));
	return [...edges.values()]
		.filter((edge) => edge.faces.length === 1)
		.map(({ points, depths }) => ({ points, depths }));
}

function normalOf(
	points: [
		[number, number, number],
		[number, number, number],
		[number, number, number],
	],
): [number, number, number] {
	const first = points[1].map((value, index) => value - points[0][index]);
	const second = points[2].map((value, index) => value - points[0][index]);
	const cross: [number, number, number] = [
		first[1] * second[2] - first[2] * second[1],
		first[2] * second[0] - first[0] * second[2],
		first[0] * second[1] - first[1] * second[0],
	];
	const length = Math.hypot(...cross) || 1;
	return cross.map((value) => value / length) as [number, number, number];
}

function dot3(
	first: readonly [number, number, number],
	second: readonly [number, number, number],
) {
	return first[0] * second[0] + first[1] * second[1] + first[2] * second[2];
}

function cameraVector(view: CadViewDirection): [number, number, number] {
	switch (view) {
		case "top_down":
			return [0, 1, 0];
		case "left_to_right":
			return [-1, 0, 0];
		case "right_to_left":
			return [1, 0, 0];
		case "front_to_back":
			return [0, 0, 1];
		case "back_to_front":
			return [0, 0, -1];
	}
}

function point3Key(point: readonly [number, number, number]) {
	return point.map((value) => Math.round(value * 20)).join(",");
}

export function rotateModelPoint(
	point: readonly [number, number, number],
	rotation: readonly [number, number, number],
): [number, number, number] {
	// Desk rotations map to renderer-world (rx, rz, ry), where the shared scene contract is
	// Rx * Ry * Rz. Applying the rightmost rotation first keeps CAD identical to the 3D renderer.
	const rx = (rotation[0] * Math.PI) / 180;
	const ry = (rotation[2] * Math.PI) / 180;
	const rz = (rotation[1] * Math.PI) / 180;
	const cosZ = Math.cos(rz);
	const sinZ = Math.sin(rz);
	const afterZ: [number, number, number] = [
		point[0] * cosZ - point[1] * sinZ,
		point[0] * sinZ + point[1] * cosZ,
		point[2],
	];
	const cosY = Math.cos(ry);
	const sinY = Math.sin(ry);
	const afterY: [number, number, number] = [
		afterZ[0] * cosY + afterZ[2] * sinY,
		afterZ[1],
		-afterZ[0] * sinY + afterZ[2] * cosY,
	];
	const cosX = Math.cos(rx);
	const sinX = Math.sin(rx);
	return [
		afterY[0],
		afterY[1] * cosX - afterY[2] * sinX,
		afterY[1] * sinX + afterY[2] * cosX,
	];
}

/**
 * A model point turned by a bracket angle about the model's transverse axis (+X), positive
 * nose-down, as the Visualizer turns a fixture without a recorded hinge (`Quat::from_rotation_x`).
 * It applies before the placement's rotation, in the lamp's own frame.
 */
export function bracketTurnedPoint(
	point: readonly [number, number, number],
	bracketAngle: number,
): [number, number, number] {
	if (!bracketAngle) return [point[0], point[1], point[2]];
	const cos = Math.cos((bracketAngle * Math.PI) / 180);
	const sin = Math.sin((bracketAngle * Math.PI) / 180);
	return [
		point[0],
		point[1] * cos - point[2] * sin,
		point[1] * sin + point[2] * cos,
	];
}

/**
 * A point in model axes (x across, y up, z toward the audience — the Visualizer's renderer axes)
 * turned into desk axes (x across, y upstage, z up): `(x, −z, y)`.
 */
export function rotateDeskPoint(
	point: readonly [number, number, number],
	rotation: readonly [number, number, number],
): [number, number, number] {
	const turned = rotateModelPoint([point[0], point[2], -point[1]], rotation);
	return [turned[0], -turned[2], turned[1]];
}

/**
 * A model point on a CAD view's page, matching `projectPoint` for the same point in desk axes: the
 * model's +Z is downstage, as the Visualizer places it (desk y = −z).
 */
function projectModelPoint(
	point: readonly [number, number, number],
	view: CadViewDirection,
): PlanPoint {
	switch (view) {
		case "top_down":
			return [point[0], -point[2]];
		case "left_to_right":
			return [point[2], point[1]];
		case "right_to_left":
			return [-point[2], point[1]];
		case "front_to_back":
			return [point[0], point[1]];
		case "back_to_front":
			return [-point[0], point[1]];
	}
}

function modelDepth(
	point: readonly [number, number, number],
	view: CadViewDirection,
): number {
	switch (view) {
		case "top_down":
			return -point[1];
		case "left_to_right":
			return point[0];
		case "right_to_left":
			return -point[0];
		case "front_to_back":
			return -point[2];
		case "back_to_front":
			return point[2];
	}
}

function entityType(entity: CadEntity): string {
	return `${entity.fixtureType} ${entity.kind} ${entity.name}`.toLowerCase();
}

function isSemanticPlanSymbol(type: string): boolean {
	return /crowd/.test(type);
}

export function parseProjection(
	svg: string,
	origin: readonly [number, number] = [0, 0],
): PlanGeometry {
	const polygons: {
		points: PlanPoint[];
		color: [number, number, number];
		outline: boolean;
	}[] = [];
	const pathPattern = /<path\b([^>]*)\/?\s*>/g;
	for (const match of svg.matchAll(pathPattern)) {
		const attributes = match[1];
		const data = attribute(attributes, "d");
		if (!data) continue;
		const numbers = data.match(/-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/gi);
		if (!numbers || numbers.length < 6 || numbers.length % 2 !== 0) continue;
		const points: PlanPoint[] = [];
		for (let index = 0; index < numbers.length; index += 2) {
			points.push([
				Number(numbers[index]) - origin[0],
				-(Number(numbers[index + 1]) - origin[1]),
			]);
		}
		polygons.push({
			points,
			color: parseHex(attribute(attributes, "fill") ?? "#66707a"),
			outline: (attribute(attributes, "data-part") ?? "").endsWith("-outline"),
		});
	}
	const selected = polygons.some((polygon) => polygon.outline)
		? polygons.filter((polygon) => polygon.outline)
		: polygons;
	const triangles: PlanTriangle[] = [];
	for (const polygon of selected)
		for (let index = 1; index < polygon.points.length - 1; index++)
			triangles.push({
				points: [
					polygon.points[0],
					polygon.points[index],
					polygon.points[index + 1],
				],
				color: polygon.color,
			});
	return {
		source: "model",
		triangles,
		outlines: [],
		lines: planarBoundaryLines(selected.map((polygon) => polygon.points)),
	};
}

function planarBoundaryLines(polygons: readonly PlanPoint[][]): PlanLine[] {
	const edges = new Map<
		string,
		{ points: [PlanPoint, PlanPoint]; count: number }
	>();
	for (const polygon of polygons) {
		for (let index = 0; index < polygon.length; index++) {
			const first = polygon[index];
			const second = polygon[(index + 1) % polygon.length];
			const firstKey = point2Key(first);
			const secondKey = point2Key(second);
			const forward = firstKey < secondKey;
			const key = forward
				? `${firstKey}|${secondKey}`
				: `${secondKey}|${firstKey}`;
			const existing = edges.get(key);
			if (existing) existing.count += 1;
			else
				edges.set(key, {
					points: forward ? [first, second] : [second, first],
					count: 1,
				});
		}
	}
	return [...edges.values()]
		.filter((edge) => edge.count === 1)
		.map((edge) => ({ points: edge.points }));
}

function point2Key(point: PlanPoint) {
	return point.map((value) => Math.round(value * 20)).join(",");
}

function attribute(source: string, name: string): string | null {
	return source.match(new RegExp(`\\s${name}="([^"]*)"`))?.[1] ?? null;
}

function parseHex(value: string): [number, number, number] {
	const match = /^#([0-9a-f]{6})$/i.exec(value);
	if (!match) return DETAIL;
	return [0, 2, 4].map(
		(offset) => Number.parseInt(match[1].slice(offset, offset + 2), 16) / 255,
	) as [number, number, number];
}

function typedGeometry(
	entity: CadEntity,
	view: CadViewDirection,
	type = entityType(entity),
): PlanGeometry {
	const [width, depth, height] = entity.sizeMillimetres;
	const horizontal =
		view === "top_down"
			? width
			: view === "front_to_back" || view === "back_to_front"
				? width
				: depth;
	const vertical = view === "top_down" ? depth : height;
	let polygons: Polygon[];

	const scenery = entity.scenery;
	if (
		scenery?.kind === "box" ||
		scenery?.kind === "cylinder" ||
		scenery?.kind === "sphere"
	) {
		polygons = [primitive(scenery.kind, horizontal, vertical, view === "top_down")];
	} else if (scenery?.kind === "flight_rack") {
		polygons = flightRackPlan(horizontal, vertical, view);
	} else if (scenery?.kind === "pa_top") {
		polygons = paSpeakerPlan(horizontal, vertical, view);
	} else if (scenery?.kind === "line_array") {
		polygons = lineArrayPlan(horizontal, vertical, view);
	} else if (scenery?.kind === "mirror_ball") {
		polygons = discoBallPlan(horizontal, vertical, view);
	} else if (
		scenery?.kind === "truss" ||
		(!scenery && /truss|pipe grid|pipe$/.test(type))
	) {
		polygons = trussPlan(
			entity.sizeMillimetres,
			view,
			scenery?.chords || trussChordCount(type),
			scenery ? scenery.pattern === "deco" : /deco/.test(type),
		);
	} else if (
		scenery?.kind === "chain" ||
		(!scenery && /chain/.test(type))
	) {
		return chainPlan(entity.sizeMillimetres, view, scenery?.chain, scenery?.anchor);
	} else if (
		scenery?.kind === "railing" ||
		(!scenery && /railing|handrail/.test(type))
	) {
		polygons = railing(horizontal, vertical, view === "top_down");
	} else if (
		view !== "top_down" &&
		// A riser from before stairs had their own kind still says stairs in its name.
		(scenery?.kind === "stairs" || /stair/.test(type))
	) {
		// A flight is drawn from the side of it the view sees once its yaw is taken into account, so
		// the steps climb the way the plan's arrow points in every elevation.
		const yaw = entity.rotationDegrees[2];
		return {
			...fromPolygons(
				"typed",
				stairElevation(width, depth, height, scenery?.handrails ?? "none", lampRelativeView(view, yaw)),
			),
			yawQuarterTurnsShown: bakedYawQuarterTurns(view, yaw),
		};
	} else if (
		scenery?.kind === "riser" ||
		scenery?.kind === "stairs" ||
		/stage element|riser|stage deck|stairs/.test(type)
	) {
		polygons = stage(
			horizontal,
			vertical,
			view === "top_down",
			// A flight of stairs is its own kind now. One from a show made before that declares
			// itself a riser and says stairs only in its name, which is how every stair was told
			// from a deck until the kind existed, so a riser that calls itself stairs still is.
			scenery?.kind === "stairs" || /stair/.test(type),
			// A deck from a show made before the decks were generated carries no scenery, and its
			// name is the only thing that says it stands on regular feet.
			scenery ? scenery.feet === "fixed" : /stage deck/.test(type),
			scenery?.handrails ?? "none",
			height,
		);
	} else if (
		scenery?.kind === "curtain" ||
		(!scenery && /curtain|drape/.test(type))
	) {
		return curtainPlan(horizontal, vertical, view);
	} else if (/crowd/.test(type)) {
		return fromOutlinePolygons(
			crowdPlan(horizontal, vertical, view, seedOf(entity.id)),
		);
	} else if (/sunstrip|pixel bar|light bar|matrix/.test(type)) {
		polygons = bar(horizontal, vertical);
	} else if (/media.server|media_server/.test(type)) {
		polygons = mediaServer(horizontal, vertical);
	} else if (/laser/.test(type)) {
		polygons = laser(horizontal, vertical);
	} else if (/spark|flame|effect/.test(type)) {
		polygons = effect(horizontal, vertical, /flame/.test(type));
	} else if (/moving|wash|beam|spot/.test(type)) {
		polygons = movingLight(horizontal, vertical, view === "top_down");
	} else if (/profile|fresnel|par|conventional|dimmer|acl|blinder/.test(type)) {
		polygons = conventional(horizontal, vertical, view === "top_down");
	} else if (entity.fixtureType && entity.fixtureType !== "fixture") {
		polygons = generalFixture(horizontal, vertical);
	} else if (entity.kind === "venue") {
		polygons = venueProp(horizontal, vertical);
	} else {
		return unknownBox(horizontal, vertical);
	}
	return fromPolygons("typed", polygons);
}

function fromOutlinePolygons(polygons: Polygon[]): PlanGeometry {
	return {
		source: "typed",
		triangles: [],
		outlines: polygons.map((polygon) => polygon.points),
		lines: [],
	};
}

function fromPolygons(
	source: PlanGeometry["source"],
	polygons: Polygon[],
): PlanGeometry {
	const triangles: PlanTriangle[] = [];
	for (const polygon of polygons) {
		for (let index = 1; index < polygon.points.length - 1; index++) {
			triangles.push({
				points: [
					polygon.points[0],
					polygon.points[index],
					polygon.points[index + 1],
				],
				color: polygon.color,
			});
		}
	}
	// A typed symbol is painted back to front, so a later polygon hides whatever earlier edges it
	// covers and nothing hidden is drawn.
	return {
		source,
		triangles,
		...hideCoveredEdges(
			polygons.map((polygon) => ({
				kind: "solid" as const,
				edges: [polygon.points],
				area: [polygon.points],
			})),
		),
	};
}

function rect(
	x: number,
	y: number,
	width: number,
	height: number,
	color: Polygon["color"],
): Polygon {
	return {
		color,
		points: [
			[x, y],
			[x + width, y],
			[x + width, y + height],
			[x, y + height],
		],
	};
}

function ellipse(
	x: number,
	y: number,
	rx: number,
	ry: number,
	color: Polygon["color"],
	segments = 16,
): Polygon {
	return {
		color,
		points: Array.from({ length: segments }, (_, index) => {
			const angle = (index / segments) * Math.PI * 2;
			return [x + Math.cos(angle) * rx, y + Math.sin(angle) * ry];
		}),
	};
}

function thickLine(
	start: PlanPoint,
	end: PlanPoint,
	thickness: number,
	color: Polygon["color"],
): Polygon {
	const dx = end[0] - start[0];
	const dy = end[1] - start[1];
	const length = Math.max(1, Math.hypot(dx, dy));
	const x = (-dy / length) * (thickness / 2);
	const y = (dx / length) * (thickness / 2);
	return {
		color,
		points: [
			[start[0] + x, start[1] + y],
			[end[0] + x, end[1] + y],
			[end[0] - x, end[1] - y],
			[start[0] - x, start[1] - y],
		],
	};
}

function movingLight(width: number, height: number, top: boolean): Polygon[] {
	const w = Math.max(220, width);
	const h = Math.max(280, height);
	if (top) {
		return [
			ellipse(0, 0, w * 0.34, h * 0.34, BASE),
			rect(-w * 0.28, -h * 0.08, w * 0.56, h * 0.16, BODY),
			ellipse(0, h * 0.2, w * 0.24, h * 0.24, DETAIL),
			ellipse(0, h * 0.29, w * 0.12, h * 0.12, DARK),
		];
	}
	// The head is deliberately painted before the near yoke arms. Their opaque polygons hide the
	// covered edge of the head, matching the side silhouette of an actual moving light.
	return [
		rect(-w * 0.34, -h * 0.48, w * 0.68, h * 0.16, BASE),
		ellipse(0, h * 0.18, w * 0.29, h * 0.25, DETAIL),
		ellipse(0, h * 0.18, w * 0.16, h * 0.14, DARK),
		rect(-w * 0.35, -h * 0.3, w * 0.1, h * 0.53, BODY),
		rect(w * 0.25, -h * 0.3, w * 0.1, h * 0.53, BODY),
		thickLine([-w * 0.3, h * 0.2], [-w * 0.15, h * 0.36], w * 0.08, BODY),
		thickLine([w * 0.3, h * 0.2], [w * 0.15, h * 0.36], w * 0.08, BODY),
	];
}

function conventional(width: number, height: number, top: boolean): Polygon[] {
	const w = Math.max(180, width);
	const h = Math.max(260, height);
	return top
		? [
				rect(-w * 0.22, -h * 0.42, w * 0.44, h * 0.58, BODY),
				{
					color: DETAIL,
					points: [
						[-w * 0.36, h * 0.16],
						[w * 0.36, h * 0.16],
						[w * 0.27, h * 0.43],
						[-w * 0.27, h * 0.43],
					],
				},
				ellipse(0, h * 0.29, w * 0.22, h * 0.12, DARK),
			]
		: [
				rect(-w * 0.32, -h * 0.36, w * 0.64, h * 0.5, BODY),
				{
					color: DETAIL,
					points: [
						[-w * 0.42, h * 0.14],
						[w * 0.42, h * 0.14],
						[w * 0.3, h * 0.4],
						[-w * 0.3, h * 0.4],
					],
				},
				ellipse(0, h * 0.27, w * 0.25, h * 0.11, DARK),
			];
}

function trussChordCount(type: string): 2 | 3 | 4 {
	if (/three[- ]point|3[- ]point|tri(?:angular)?/.test(type)) return 3;
	if (/two[- ]point|2[- ]point|ladder/.test(type)) return 2;
	return 4;
}

function stage(
	width: number,
	height: number,
	top: boolean,
	stairs: boolean,
	fixedFeet: boolean,
	handrails: CadStairHandrails = "none",
	rise = height,
): Polygon[] {
	const w = Math.max(300, width);
	const h = Math.max(120, height);
	if (top && stairs) return stairPlan(w, h, rise, handrails);
	if (top)
		return [
			rect(-w / 2, -h / 2, w, h, BODY),
			rect(-w / 2 + 35, -h / 2 + 35, w - 70, h - 70, BASE),
		];
	// In an elevation a stage element stands on its origin, the floor its feet are on, and rises
	// its height from there, as it does in the Visualizer.
	return fixedFeet ? leggedStage(w, h) : scissorStage(w, h);
}

/** The section a rail and its posts are drawn at. */
const RAIL_SECTION = 40;

/**
 * A handrail: posts along its run with a top rail and a knee rail, the way `push_railing` builds
 * it. Seen from above it is the thin line of its own section.
 */
function railing(width: number, height: number, top: boolean): Polygon[] {
	const w = Math.max(200, width);
	const h = Math.max(200, height);
	if (top) return [rect(-w / 2, -Math.max(RAIL_SECTION, height) / 2, w, Math.max(RAIL_SECTION, height), BODY)];
	// A railing stands on the floor it is placed on, like a stage element, so its rails are
	// measured up from the origin rather than from the middle of a box.
	const posts = Math.min(40, Math.max(2, Math.round(w / 1200)));
	const polygons: Polygon[] = [];
	for (let index = 0; index <= posts; index += 1)
		polygons.push(rect(-w / 2 + (w * index) / posts - RAIL_SECTION / 2, 0, RAIL_SECTION, h, BODY));
	for (const at of [h - RAIL_SECTION, h * 0.55])
		polygons.push(rect(-w / 2, at, w, RAIL_SECTION, DETAIL));
	return polygons;
}

/** The top a deck on regular feet is drawn with, and the section of one leg, in millimetres. */
const DECK_TOP = 40;
const DECK_LEG = 60;

/**
 * A deck on regular feet from the front or side: its top at the height it is placed, carried by
 * one leg under each corner, the way `push_fixed_legs` builds it in the Visualizer.
 */
function leggedStage(w: number, h: number): Polygon[] {
	const top = Math.min(DECK_TOP, h * 0.4);
	const leg = Math.min(DECK_LEG, w * 0.4);
	const rise = h - top;
	const polygons: Polygon[] = [];
	// A leg stands under a corner with its outer face flush with the deck's edge, as the legs are
	// built in the Visualizer.
	if (rise > 0)
		for (const left of [-w / 2, w / 2 - leg])
			polygons.push(rect(left, 0, leg, rise, BODY));
	polygons.push(rect(-w / 2, rise, w, top, DETAIL));
	return polygons;
}

/** Steepest a scissor arm is drawn, from horizontal; a taller rise stacks another X. */
export const SCISSOR_MAX_DEGREES = 40;

/**
 * A stage element from the front or side: its deck on top, a base frame on the floor and scissor
 * arms crossed between them, in as many stacked stages as keep each arm at 40° or flatter. The base
 * frame stands on the origin and the deck's top is the element's height above it.
 */
function scissorStage(w: number, h: number): Polygon[] {
	const deck = Math.min(80, h * 0.3);
	const base = Math.min(50, h * 0.2);
	const arm = Math.max(8, Math.min(40, h * 0.06, w * 0.03));
	// The arms reach almost to the deck's edges, as a lift under a whole deck does.
	const span = w - arm * 2;
	const bottom = base;
	const top = h - deck;
	const rise = top - bottom;
	const tan = Math.tan((SCISSOR_MAX_DEGREES * Math.PI) / 180);
	const stages = Math.max(1, Math.ceil(rise / (span * tan) - 1e-9));
	const step = rise / stages;
	const polygons: Polygon[] = [];
	for (let stage = 0; stage < stages; stage++) {
		const low = bottom + stage * step;
		const high = low + step;
		const [left, right] = [-span / 2, span / 2];
		// An arm's ends sit inside the deck and the base frame, so they are cut where those begin
		// and no end of an arm is drawn across either of them.
		for (const arm_ of [
			thickLine([left, low], [right, high], arm, BODY),
			thickLine([left, high], [right, low], arm, BODY),
		])
			polygons.push({ ...arm_, points: clipBetween(arm_.points, bottom, top) });
		polygons.push(ellipse(0, (low + high) / 2, arm * 0.45, arm * 0.45, DARK, 10));
	}
	polygons.push(
		rect(-w / 2, top, w, deck, DETAIL),
		rect(-w / 2, 0, w, base, BODY),
	);
	return polygons;
}

/** The part of a convex polygon between two heights. */
function clipBetween(points: PlanPoint[], low: number, high: number): PlanPoint[] {
	const clip = (input: PlanPoint[], inside: (y: number) => boolean, edge: number) => {
		const output: PlanPoint[] = [];
		input.forEach((current, index) => {
			const previous = input[(index + input.length - 1) % input.length];
			const crossing = (): PlanPoint => {
				const t = (edge - previous[1]) / (current[1] - previous[1]);
				return [previous[0] + (current[0] - previous[0]) * t, edge];
			};
			if (inside(current[1])) {
				if (!inside(previous[1])) output.push(crossing());
				output.push(current);
			} else if (inside(previous[1])) {
				output.push(crossing());
			}
		});
		return output;
	};
	return clip(
		clip(points, (y) => y >= low, low),
		(y) => y <= high,
		high,
	);
}

function bar(width: number, height: number): Polygon[] {
	const w = Math.max(400, width);
	const h = Math.max(100, height);
	const polygons = [rect(-w / 2, -h * 0.22, w, h * 0.44, BASE)];
	const cells = 10;
	for (let index = 0; index < cells; index++) {
		polygons.push(
			ellipse(
				-w * 0.44 + (index / (cells - 1)) * w * 0.88,
				0,
				h * 0.14,
				h * 0.14,
				DETAIL,
				10,
			),
		);
	}
	return polygons;
}

function mediaServer(width: number, height: number): Polygon[] {
	const w = Math.max(360, width);
	const h = Math.max(500, height);
	const polygons = [rect(-w / 2, -h / 2, w, h, BASE)];
	for (let row = 0; row < 5; row++) {
		polygons.push(
			rect(
				-w * 0.42,
				-h * 0.39 + row * h * 0.18,
				w * 0.84,
				h * 0.1,
				row === 1 ? DETAIL : BODY,
			),
		);
	}
	return polygons;
}

function laser(width: number, height: number): Polygon[] {
	const w = Math.max(220, width);
	const h = Math.max(180, height);
	return [
		{
			color: BASE,
			points: [
				[-w / 2, -h / 2],
				[w / 2, -h / 2],
				[w * 0.38, h / 2],
				[-w * 0.38, h / 2],
			],
		},
		ellipse(0, h * 0.18, w * 0.12, h * 0.12, DETAIL, 12),
		{
			color: DARK,
			points: [
				[-w * 0.08, h * 0.18],
				[w * 0.08, h * 0.18],
				[0, h * 0.46],
			],
		},
	];
}

function effect(width: number, height: number, flame: boolean): Polygon[] {
	const w = Math.max(180, width);
	const h = Math.max(260, height);
	const plume: Polygon = flame
		? {
				color: DETAIL,
				points: [
					[-w * 0.2, -h * 0.05],
					[0, h * 0.5],
					[w * 0.2, -h * 0.05],
					[0, h * 0.16],
				],
			}
		: {
				color: DETAIL,
				points: [
					[-w * 0.34, 0],
					[0, h * 0.5],
					[w * 0.34, 0],
					[0, h * 0.18],
				],
			};
	return [rect(-w * 0.38, -h * 0.5, w * 0.76, h * 0.32, BASE), plume];
}

function generalFixture(width: number, height: number): Polygon[] {
	const w = Math.max(180, width);
	const h = Math.max(180, height);
	return [
		ellipse(0, 0, w * 0.42, h * 0.42, BODY),
		ellipse(0, h * 0.06, w * 0.25, h * 0.25, DETAIL),
		ellipse(0, h * 0.08, w * 0.12, h * 0.12, DARK),
	];
}

/**
 * A primitive shape at its own size, filling the box it is placed at: a box is its rectangle in
 * every view, an upright cylinder is its ellipse from above and a rectangle from any side, and a ball
 * is its ellipse in every view.
 */
function primitive(
	kind: "box" | "cylinder" | "sphere",
	width: number,
	height: number,
	top: boolean,
): Polygon {
	const round = kind === "sphere" || (kind === "cylinder" && top);
	return round
		? ellipse(0, 0, width / 2, height / 2, BODY, 32)
		: rect(-width / 2, -height / 2, width, height, BODY);
}

function venueProp(width: number, height: number): Polygon[] {
	return [
		ellipse(
			0,
			0,
			Math.max(120, width / 2),
			Math.max(120, height / 2),
			BODY,
			20,
		),
		ellipse(
			0,
			0,
			Math.max(60, width * 0.3),
			Math.max(60, height * 0.3),
			BASE,
			20,
		),
	];
}

function unknownBox(width: number, height: number): PlanGeometry {
	const w = Math.max(180, width);
	const h = Math.max(180, height);
	const polygon = rect(-w / 2, -h / 2, w, h, BASE);
	const geometry = fromPolygons("unknown", [polygon]);
	geometry.outlines.push([
		[-w / 2, -h / 2],
		[w / 2, h / 2],
	]);
	geometry.outlines.push([
		[-w / 2, h / 2],
		[w / 2, -h / 2],
	]);
	return geometry;
}
