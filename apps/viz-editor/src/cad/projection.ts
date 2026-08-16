import type {
	CadDrawing,
	CadEntity,
	CadProjectionView,
	CadViewDirection,
} from "./types";

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
	source: "live_model" | "model" | "typed" | "unknown";
	triangles: PlanTriangle[];
	outlines: PlanPoint[][];
	lines: PlanLine[];
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

export function entityPlanGeometry(
	entity: CadEntity,
	drawing: CadDrawing | undefined,
	view: CadViewDirection,
): PlanGeometry {
	const type = entityType(entity);
	// Crowd-area models describe a procedural volume and read as an unexplained block in plan.
	// Modeled fixtures and venue objects—including trusses—keep their canonical generated SVG.
	if (isSemanticPlanSymbol(type)) return typedGeometry(entity, view, type);
	const live = liveModelGeometry(entity, drawing, view);
	if (live) return live;
	const projection = drawing?.projections.find(
		(candidate) => candidate.view === projectionViewForCad(view),
	);
	if (projection) {
		const parsed = parseProjection(
			projection.svg,
			projection.originMillimetres,
		);
		if (parsed.triangles.length) return parsed;
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
				rotateModelPoint(point, entity.rotationDegrees),
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

function rotateModelPoint(
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

	if (/truss|pipe grid|pipe$/.test(type)) {
		polygons = truss(horizontal, vertical, trussChordCount(type));
	} else if (/stage element|riser|stage deck|stairs/.test(type)) {
		polygons = stage(horizontal, vertical, view === "top_down");
	} else if (/curtain|drape/.test(type)) {
		polygons = curtain(horizontal, vertical);
	} else if (/crowd/.test(type)) {
		polygons = crowd(horizontal, vertical, view);
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
	return {
		source,
		triangles,
		outlines: polygons.map((polygon) => polygon.points),
		lines: [],
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

function truss(
	width: number,
	height: number,
	chordCount: 2 | 3 | 4,
): Polygon[] {
	const crossSection = width / Math.max(1, height) < 1.7;
	const w = Math.max(500, width);
	const h = Math.max(180, height);
	const diameter = Math.max(24, Math.min(70, h * 0.16));

	// Looking along the truss shows its declared chord arrangement, not one filled box.
	if (crossSection) {
		const radius = Math.min(w, h) * 0.34;
		const centres: PlanPoint[] =
			chordCount === 2
				? [
						[0, -radius],
						[0, radius],
					]
				: chordCount === 3
					? [
							[0, -radius],
							[-radius * 0.86, radius * 0.55],
							[radius * 0.86, radius * 0.55],
						]
					: [
							[-radius, -radius],
							[radius, -radius],
							[radius, radius],
							[-radius, radius],
						];
		const polygons: Polygon[] = [];
		for (let index = 0; index < centres.length; index++) {
			polygons.push(
				thickLine(
					centres[index],
					centres[(index + 1) % centres.length],
					diameter * 0.45,
					BODY,
				),
			);
		}
		for (const [x, y] of centres) {
			polygons.push(ellipse(x, y, diameter / 2, diameter / 2, DETAIL, 14));
		}
		return polygons;
	}

	const polygons = [
		rect(-w / 2, -h / 2, w, diameter, DETAIL),
		rect(-w / 2, h / 2 - diameter, w, diameter, DETAIL),
	];
	const bays = Math.max(2, Math.min(12, Math.round(w / Math.max(400, h))));
	for (let index = 0; index < bays; index++) {
		const x0 = -w / 2 + (index / bays) * w;
		const x1 = -w / 2 + ((index + 1) / bays) * w;
		polygons.push(
			thickLine(
				[x0, -h / 2 + diameter],
				[x1, h / 2 - diameter],
				diameter * 0.45,
				BODY,
			),
			thickLine(
				[x0, h / 2 - diameter],
				[x1, -h / 2 + diameter],
				diameter * 0.45,
				BODY,
			),
		);
	}
	return polygons;
}

function stage(width: number, height: number, top: boolean): Polygon[] {
	const w = Math.max(300, width);
	const h = Math.max(120, height);
	if (top)
		return [
			rect(-w / 2, -h / 2, w, h, BODY),
			rect(-w / 2 + 35, -h / 2 + 35, w - 70, h - 70, BASE),
		];
	return [
		rect(-w / 2, h * 0.25, w, h * 0.22, DETAIL),
		rect(-w * 0.44, -h * 0.48, w * 0.08, h * 0.73, BODY),
		rect(w * 0.36, -h * 0.48, w * 0.08, h * 0.73, BODY),
	];
}

function curtain(width: number, height: number): Polygon[] {
	const w = Math.max(300, width);
	const h = Math.max(300, height);
	const folds = 10;
	return Array.from({ length: folds }, (_, index) => {
		const left = -w / 2 + (index / folds) * w;
		const right = -w / 2 + ((index + 1) / folds) * w;
		return {
			color: index % 2 ? BASE : BODY,
			points: [
				[left, -h / 2],
				[right, -h / 2],
				[right - (w / folds) * 0.16, h / 2],
				[left + (w / folds) * 0.16, h / 2],
			],
		};
	});
}

function crowd(
	width: number,
	height: number,
	view: CadViewDirection,
): Polygon[] {
	const w = Math.max(600, width);
	const h = Math.max(500, height);
	const polygons: Polygon[] = [];
	for (let row = 0; row < 4; row++) {
		for (let column = 0; column < 6; column++) {
			const x = -w * 0.4 + (column / 5) * w * 0.8 + (row % 2 ? w * 0.04 : 0);
			const y = -h * 0.38 + (row / 3) * h * 0.76;
			const personWidth = Math.min(w * 0.07, h * 0.1);
			const personHeight = Math.min(h * 0.14, w * 0.1);
			if (view === "top_down") {
				const headRadius = personWidth * 0.22;
				const bodyWidth = personWidth;
				const bodyHeight = personHeight * 0.5;
				polygons.push(
					ellipse(x, y - bodyHeight * 0.72, headRadius, headRadius, DETAIL, 10),
					roundedRect(
						x,
						y + bodyHeight * 0.08,
						bodyWidth,
						bodyHeight,
						Math.min(bodyWidth, bodyHeight) * 0.28,
						BODY,
					),
					segment(
						[x - bodyWidth * 0.22, y + bodyHeight * 0.28],
						[x - bodyWidth * 0.42, y + bodyHeight * 0.62],
						personWidth * 0.12,
						BODY,
					),
					segment(
						[x + bodyWidth * 0.22, y + bodyHeight * 0.28],
						[x + bodyWidth * 0.42, y + bodyHeight * 0.62],
						personWidth * 0.12,
						BODY,
					),
				);
			} else if (view === "left_to_right" || view === "right_to_left") {
				const direction = view === "left_to_right" ? 1 : -1;
				const thickness = personWidth * 0.12;
				const shoulder: PlanPoint = [x, y - personHeight * 0.08];
				const hip: PlanPoint = [x, y + personHeight * 0.22];
				polygons.push(
					ellipse(
						x,
						y - personHeight * 0.34,
						personWidth * 0.22,
						personWidth * 0.22,
						DETAIL,
						10,
					),
					segment(shoulder, hip, thickness, BODY),
					segment(
						shoulder,
						[x + direction * personWidth * 0.62, y + personHeight * 0.02],
						thickness,
						BODY,
					),
					segment(
						[x, y],
						[x + direction * personWidth * 0.58, y + personHeight * 0.12],
						thickness,
						BODY,
					),
					segment(
						hip,
						[x - personWidth * 0.08, y + personHeight * 0.48],
						thickness,
						BODY,
					),
					segment(
						hip,
						[x + personWidth * 0.08, y + personHeight * 0.48],
						thickness,
						BODY,
					),
				);
			} else {
				const thickness = personWidth * 0.11;
				const shoulder: PlanPoint = [x, y - personHeight * 0.08];
				const hip: PlanPoint = [x, y + personHeight * 0.2];
				polygons.push(
					ellipse(
						x,
						y - personHeight * 0.34,
						personWidth * 0.22,
						personWidth * 0.22,
						DETAIL,
						10,
					),
					segment(shoulder, hip, thickness, BODY),
					segment(
						shoulder,
						[x - personWidth * 0.58, y + personHeight * 0.08],
						thickness,
						BODY,
					),
					segment(
						shoulder,
						[x + personWidth * 0.58, y + personHeight * 0.08],
						thickness,
						BODY,
					),
					segment(
						hip,
						[x - personWidth * 0.32, y + personHeight * 0.48],
						thickness,
						BODY,
					),
					segment(
						hip,
						[x + personWidth * 0.32, y + personHeight * 0.48],
						thickness,
						BODY,
					),
				);
			}
		}
	}
	return polygons;
}

function segment(
	start: PlanPoint,
	end: PlanPoint,
	thickness: number,
	color: Polygon["color"],
): Polygon {
	const dx = end[0] - start[0];
	const dy = end[1] - start[1];
	const length = Math.max(0.001, Math.hypot(dx, dy));
	const nx = (-dy / length) * (thickness / 2);
	const ny = (dx / length) * (thickness / 2);
	return {
		color,
		points: [
			[start[0] + nx, start[1] + ny],
			[end[0] + nx, end[1] + ny],
			[end[0] - nx, end[1] - ny],
			[start[0] - nx, start[1] - ny],
		],
	};
}

function roundedRect(
	cx: number,
	cy: number,
	width: number,
	height: number,
	radius: number,
	color: Polygon["color"],
): Polygon {
	const r = Math.min(radius, width / 2, height / 2);
	const points: PlanPoint[] = [];
	for (const [cornerX, cornerY, start] of [
		[cx + width / 2 - r, cy + height / 2 - r, 0],
		[cx - width / 2 + r, cy + height / 2 - r, Math.PI / 2],
		[cx - width / 2 + r, cy - height / 2 + r, Math.PI],
		[cx + width / 2 - r, cy - height / 2 + r, Math.PI * 1.5],
	] as const) {
		for (let index = 0; index <= 3; index++) {
			const angle = start + (index / 3) * (Math.PI / 2);
			points.push([
				cornerX + Math.cos(angle) * r,
				cornerY + Math.sin(angle) * r,
			]);
		}
	}
	return { color, points };
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
