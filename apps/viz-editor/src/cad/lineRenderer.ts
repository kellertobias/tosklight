/**
 * The WebGL line renderer behind every CAD viewport.
 *
 * One frame is built into three vertex arrays and drawn in three passes: opaque depth masks in the
 * background colour, hidden-line-aware linework, and finally the overlay linework — gizmo, guides
 * and marquee — which is drawn without the depth test so it is never occluded by the rig.
 *
 * Vertices reach the shader already in clip space, so every painter below works in plan
 * millimetres and the camera is applied once, in `painterFor`.
 */
import { entityPlanGeometry, type PlanGeometry, type PlanPoint } from "./projection";
import {
	gizmoGeometry,
	type MoveAxis,
	viewDepth,
	viewPositionDepth,
	worldGeometry,
	viewportGuideRange,
} from "./planGeometry";
import { annotationRuns } from "./annotationGeometry";
import type { CadAnnotation } from "./annotations";
import { placedPolylines } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";
import type {
	CadDrawing,
	CadEntity,
	CadTransformPreview,
	CadViewDirection,
	TileCamera,
	WorldAxis,
} from "./types";
import {
	directionIndicator,
	previewDeltaForEntity,
	projectPoint,
	viewAxes,
} from "./types";

/** Red, green, blue and an optional opacity, which defaults to opaque. */
export type LineColor =
	| [number, number, number]
	| [number, number, number, number];

/** One vertex: a clip-space position and an RGBA colour. */
const VERTEX_FLOATS = 7;
const VERTEX_BYTES = VERTEX_FLOATS * Float32Array.BYTES_PER_ELEMENT;

/** A fixture's beam direction: yellow at half strength, so it reads without outshouting the rig. */
const DIRECTION_COLOR: LineColor = [0.98, 0.85, 0.2, 0.5];

/** The marquee rectangle in plan millimetres, while a selection drag is in flight. */
export interface SelectionBox {
	start: [number, number];
	end: [number, number];
}

/** Everything one drawn frame of a CAD viewport depends on. */
export interface CadFrame {
	entities: readonly CadEntity[];
	drawings: ReadonlyMap<string, CadDrawing>;
	selected: ReadonlySet<string>;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	preview: CadTransformPreview | null;
	editEnabled: boolean;
	guide: MoveAxis | null;
	selectionBox: SelectionBox | null;
	showCoordinateOrigins?: boolean;
	/** Venue drawings placed on this view, drawn under the rig. */
	underlays?: readonly CadUnderlay[];
	/** Lines, boxes and measurements drawn on this view, over the rig; `draft` is one in progress. */
	annotations?: readonly CadAnnotation[];
	/** Where a move or a measurement has snapped onto a fit, marked over everything. */
	snapMarkers?: readonly PlanPoint[];
}

/** The three vertex arrays of a frame, and the closures that append plan points to them. */
interface Painter {
	fillVertices: number[];
	depthLineVertices: number[];
	lineVertices: number[];
	/** Filled shapes drawn over everything, such as the move gizmo's arrows. */
	overlayVertices: number[];
	/** One device pixel, in plan millimetres at this camera. */
	pixel: number;
	vertex(
		vertices: number[],
		point: PlanPoint,
		color: LineColor,
		depth?: number,
	): void;
	line(a: PlanPoint, b: PlanPoint, color: LineColor): void;
	/** A filled triangle over everything. */
	triangle(a: PlanPoint, b: PlanPoint, c: PlanPoint, color: LineColor): void;
	/** A line `width` plan millimetres wide, drawn as a filled strip over everything. */
	stroke(a: PlanPoint, b: PlanPoint, color: LineColor, width: number): void;
	depthLine(
		a: PlanPoint,
		b: PlanPoint,
		color: LineColor,
		firstDepth: number,
		secondDepth: number,
	): void;
}

function painterFor(canvas: HTMLCanvasElement, camera: TileCamera): Painter {
	const fillVertices: number[] = [];
	const depthLineVertices: number[] = [];
	const lineVertices: number[] = [];
	const overlayVertices: number[] = [];
	const vertex = (
		vertices: number[],
		point: PlanPoint,
		color: LineColor,
		depth = 0,
	) => {
		vertices.push(
			((point[0] + camera.pan[0]) * camera.zoom * 2) / canvas.clientWidth,
			((point[1] + camera.pan[1]) * camera.zoom * 2) / canvas.clientHeight,
			Math.max(-0.999, Math.min(0.999, depth / 100_000)),
			color[0],
			color[1],
			color[2],
			color[3] ?? 1,
		);
	};
	const triangle = (a: PlanPoint, b: PlanPoint, c: PlanPoint, color: LineColor) => {
		vertex(overlayVertices, a, color);
		vertex(overlayVertices, b, color);
		vertex(overlayVertices, c, color);
	};
	return {
		fillVertices,
		depthLineVertices,
		lineVertices,
		overlayVertices,
		pixel: 1 / ((window.devicePixelRatio || 1) * camera.zoom),
		vertex,
		line: (a, b, color) => {
			vertex(lineVertices, a, color);
			vertex(lineVertices, b, color);
		},
		triangle,
		stroke: (a, b, color, width) => {
			const length = Math.max(0.0001, Math.hypot(b[0] - a[0], b[1] - a[1]));
			const nx = (-(b[1] - a[1]) / length) * (width / 2);
			const ny = ((b[0] - a[0]) / length) * (width / 2);
			const a1: PlanPoint = [a[0] + nx, a[1] + ny];
			const a2: PlanPoint = [a[0] - nx, a[1] - ny];
			const b1: PlanPoint = [b[0] + nx, b[1] + ny];
			const b2: PlanPoint = [b[0] - nx, b[1] - ny];
			triangle(a1, a2, b1, color);
			triangle(b1, a2, b2, color);
		},
		depthLine: (a, b, color, firstDepth, secondDepth) => {
			vertex(depthLineVertices, a, color, firstDepth - 1);
			vertex(depthLineVertices, b, color, secondDepth - 1);
		},
	};
}

export function cadEntityOutlineColor(
	entity: Pick<CadEntity, "kind" | "selectable">,
	active: boolean,
): LineColor {
	if (active) return [0.02, 0.82, 0.98];
	if (!entity.selectable) return [0.24, 0.27, 0.3];
	return entity.kind === "venue" ? [0.56, 0.62, 0.68] : [0.8, 0.84, 0.88];
}

export function renderDepthMaskedLinework(
	gl: Pick<
		WebGL2RenderingContext,
		| "DEPTH_TEST"
		| "LEQUAL"
		| "POLYGON_OFFSET_FILL"
		| "enable"
		| "disable"
		| "depthFunc"
		| "polygonOffset"
	>,
	drawMasks: () => void,
	drawLines: () => void,
) {
	gl.enable(gl.DEPTH_TEST);
	gl.depthFunc(gl.LEQUAL);
	// The masks and their technical outlines describe the same physical surface. Offset only the
	// invisible masks so coplanar neighbouring decks cannot erase one another's exposed edges;
	// genuinely nearer geometry remains in front and still suppresses hidden linework.
	gl.enable(gl.POLYGON_OFFSET_FILL);
	gl.polygonOffset(1, 1);
	drawMasks();
	gl.disable(gl.POLYGON_OFFSET_FILL);
	drawLines();
	gl.disable(gl.DEPTH_TEST);
}

/** The datum guide of an elevation, and the coordinate axes when they are switched on. */
function paintDatum(painter: Painter, frame: CadFrame, canvas: HTMLCanvasElement) {
	const { view, rotationQuarterTurns, camera } = frame;
	const worldOrigin = projectPoint([0, 0, 0], view, rotationQuarterTurns);
	if (view !== "top_down") {
		dottedGuide(
			painter.line,
			worldOrigin,
			true,
			[0.22, 0.25, 0.28],
			camera,
			canvas,
		);
	}
	if (!frame.showCoordinateOrigins) return;
	// Plain lines with no heads, so the origin never reads as the move gizmo.
	const axes = viewAxes(view, rotationQuarterTurns);
	const length = 27 / camera.zoom;
	painter.line(
		worldOrigin,
		[worldOrigin[0] + length * axes.horizontal.sign, worldOrigin[1]],
		axisColor(axes.horizontal.axis),
	);
	painter.line(
		worldOrigin,
		[worldOrigin[0], worldOrigin[1] + length * axes.vertical.sign],
		axisColor(axes.vertical.axis),
	);
}

/**
 * The venue drawings placed on this view, under the rig.
 *
 * They are drawn in the overlay pass with no depth of their own, in a dimmer grey than the rig, so
 * a plan reads as the paper the rig sits on rather than as another object in it.
 */
function paintUnderlays(painter: Painter, frame: CadFrame) {
	const colour: LineColor = [0.3, 0.34, 0.38];
	for (const underlay of frame.underlays ?? []) {
		for (const run of placedPolylines(underlay, frame.rotationQuarterTurns)) {
			for (let index = 1; index < run.length; index++)
				painter.line(run[index - 1], run[index], colour);
		}
	}
}

/**
 * What the operator drew on this view, over the rig: lines and boxes in a pale grey, measurements in
 * amber, and the item still being drawn in the selection cyan.
 */
function paintAnnotations(painter: Painter, frame: CadFrame) {
	for (const annotation of frame.annotations ?? []) {
		const colour: LineColor =
			annotation.id === "draft"
				? [0.02, 0.82, 0.98]
				: annotation.kind === "measure"
					? [0.98, 0.72, 0.2]
					: [0.86, 0.89, 0.92];
		for (const run of annotationRuns(annotation, frame.rotationQuarterTurns))
			for (let index = 1; index < run.length; index++)
				painter.line(run[index - 1], run[index], colour);
	}
}

/** The rig itself: every entity's drawing, its depth mask, its outline and its beam direction. */
function paintEntities(
	painter: Painter,
	frame: CadFrame,
	geometryCache: Map<string, PlanGeometry>,
) {
	const { view, rotationQuarterTurns, drawings, selected, preview } = frame;
	const ordered = [...frame.entities].sort(
		(left, right) => viewDepth(left, view) - viewDepth(right, view),
	);
	for (const entity of ordered) {
		const active = selected.has(entity.logicalFixtureId);
		const entityWorldPreview = previewDeltaForEntity(
			preview,
			entity.logicalFixtureId,
		);
		const entityPreview = projectPoint(
			entityWorldPreview,
			view,
			rotationQuarterTurns,
		);
		const drawing = drawings.get(entity.drawingId);
		// The viewport always draws mounting hardware; the key says so, because a print page's
		// geometry for the same fixture can differ in exactly that.
		const key = `${entity.drawingId}:${view}:${entity.sizeMillimetres.join(",")}:${entity.rotationDegrees.join(",")}:hardware`;
		let geometry = geometryCache.get(key);
		if (!geometry) {
			geometry = entityPlanGeometry(entity, drawing, view);
			geometryCache.set(key, geometry);
		}
		const projected = worldGeometry(
			entity,
			geometry,
			view,
			rotationQuarterTurns,
			entityPreview,
		);
		const baseDepth =
			viewPositionDepth(entity.positionMillimetres, view) +
			viewPositionDepth(entityWorldPreview, view);
		for (const triangle of projected.triangles) {
			for (let index = 0; index < triangle.points.length; index++)
				painter.vertex(
					painter.fillVertices,
					triangle.points[index],
					[0.018, 0.024, 0.032],
					baseDepth + (triangle.depths?.[index] ?? 0),
				);
		}
		const outlineColor = cadEntityOutlineColor(entity, active);
		for (const outline of projected.outlines) {
			for (let index = 0; index < outline.length; index++) {
				painter.depthLine(
					outline[index],
					outline[(index + 1) % outline.length],
					outlineColor,
					baseDepth,
					baseDepth,
				);
			}
		}
		for (const modelLine of projected.lines)
			painter.depthLine(
				modelLine.points[0],
				modelLine.points[1],
				outlineColor,
				baseDepth + (modelLine.depths?.[0] ?? 0),
				baseDepth + (modelLine.depths?.[1] ?? 0),
			);
		const centre = projectPoint(
			entity.positionMillimetres,
			view,
			rotationQuarterTurns,
		);
		centre[0] += entityPreview[0];
		centre[1] += entityPreview[1];
		if (entity.kind !== "venue") {
			const [start, end] = directionIndicator(
				entity,
				centre,
				view,
				rotationQuarterTurns,
			);
			painter.line(start, end, DIRECTION_COLOR);
		}
	}
}

/**
 * The move gizmo at the selection's origin, and the axis guide a constrained drag follows. Its
 * arrows are two pixels wide with filled heads, so they stand out from the one-pixel rig.
 */
function paintGizmo(painter: Painter, frame: CadFrame, canvas: HTMLCanvasElement) {
	const { view, rotationQuarterTurns, camera, preview, guide } = frame;
	const gizmo = frame.editEnabled
		? gizmoGeometry(
				frame.entities,
				frame.selected,
				view,
				rotationQuarterTurns,
				camera,
				preview
					? projectPoint(preview.deltaMillimetres, view, rotationQuarterTurns)
					: [0, 0],
			)
		: null;
	if (!gizmo) return;
	const axes = viewAxes(view, rotationQuarterTurns);
	const horizontal = axisColor(axes.horizontal.axis);
	const vertical = axisColor(axes.vertical.axis);
	const { origin, length, square } = gizmo;
	const handle: LineColor = [0.75, 0.8, 0.84];
	const width = 2 * painter.pixel;
	const corners: PlanPoint[] = [
		[origin[0] - square, origin[1] - square],
		[origin[0] + square, origin[1] - square],
		[origin[0] + square, origin[1] + square],
		[origin[0] - square, origin[1] + square],
	];
	corners.forEach((corner, index) =>
		painter.line(corner, corners[(index + 1) % corners.length], handle),
	);
	drawGizmoArrow(painter, origin, [origin[0] + length, origin[1]], horizontal, square, width);
	drawGizmoArrow(painter, origin, [origin[0], origin[1] + length], vertical, square, width);
	if (guide === "horizontal")
		dottedGuide(painter.line, origin, true, horizontal, camera, canvas);
	if (guide === "vertical")
		dottedGuide(painter.line, origin, false, vertical, camera, canvas);
}

function paintSelectionBox(painter: Painter, frame: CadFrame) {
	if (!frame.selectionBox) return;
	const { start, end } = frame.selectionBox;
	const color: LineColor = [0.02, 0.82, 0.98];
	painter.line([start[0], start[1]], [end[0], start[1]], color);
	painter.line([end[0], start[1]], [end[0], end[1]], color);
	painter.line([end[0], end[1]], [start[0], end[1]], color);
	painter.line([start[0], end[1]], [start[0], start[1]], color);
}

/** A snapped fit: a magenta diamond eight pixels across, so it reads apart from the cyan selection. */
function paintSnapMarkers(painter: Painter, frame: CadFrame) {
	const color: LineColor = [1, 0.3, 0.85];
	const size = 8 * painter.pixel * (window.devicePixelRatio || 1);
	for (const [x, y] of frame.snapMarkers ?? []) {
		const corners: PlanPoint[] = [
			[x, y + size],
			[x + size, y],
			[x, y - size],
			[x - size, y],
		];
		corners.forEach((corner, index) =>
			painter.stroke(corner, corners[(index + 1) % corners.length], color, 2 * painter.pixel),
		);
	}
}

export class LineRenderer {
	private readonly geometryCache = new Map<string, PlanGeometry>();

	private constructor(
		private readonly canvas: HTMLCanvasElement,
		private readonly gl: WebGL2RenderingContext,
		private readonly program: WebGLProgram,
		private readonly buffer: WebGLBuffer,
	) {}

	static create(canvas: HTMLCanvasElement): LineRenderer | null {
		const gl = canvas.getContext("webgl2", { antialias: true });
		if (!gl) return null;
		const vertex = shader(
			gl,
			gl.VERTEX_SHADER,
			`#version 300 es
			in vec3 position; in vec4 color; out vec4 lineColor;
			void main(){ gl_Position=vec4(position,1.0); lineColor=color; }`,
		);
		const fragment = shader(
			gl,
			gl.FRAGMENT_SHADER,
			`#version 300 es
			precision mediump float; in vec4 lineColor; out vec4 outputColor;
			void main(){ outputColor=lineColor; }`,
		);
		if (!vertex || !fragment) return null;
		const program = gl.createProgram();
		const buffer = gl.createBuffer();
		if (!program || !buffer) return null;
		gl.attachShader(program, vertex);
		gl.attachShader(program, fragment);
		gl.linkProgram(program);
		if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return null;
		return new LineRenderer(canvas, gl, program, buffer);
	}

	resize() {
		const ratio = window.devicePixelRatio || 1;
		const width = Math.max(1, Math.round(this.canvas.clientWidth * ratio));
		const height = Math.max(1, Math.round(this.canvas.clientHeight * ratio));
		if (this.canvas.width !== width || this.canvas.height !== height) {
			this.canvas.width = width;
			this.canvas.height = height;
		}
	}

	draw(frame: CadFrame) {
		this.resize();
		const gl = this.gl;
		gl.viewport(0, 0, this.canvas.width, this.canvas.height);
		gl.clearColor(0.018, 0.024, 0.032, 1);
		gl.clearDepth(1);
		gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
		const painter = painterFor(this.canvas, frame.camera);
		paintDatum(painter, frame, this.canvas);
		paintUnderlays(painter, frame);
		paintEntities(painter, frame, this.geometryCache);
		paintAnnotations(painter, frame);
		paintGizmo(painter, frame, this.canvas);
		paintSelectionBox(painter, frame);
		paintSnapMarkers(painter, frame);
		this.upload(painter);
	}

	private upload(painter: Painter) {
		const gl = this.gl;
		gl.useProgram(this.program);
		gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
		const position = gl.getAttribLocation(this.program, "position");
		const color = gl.getAttribLocation(this.program, "color");
		gl.enableVertexAttribArray(position);
		gl.vertexAttribPointer(position, 3, gl.FLOAT, false, VERTEX_BYTES, 0);
		gl.enableVertexAttribArray(color);
		gl.vertexAttribPointer(color, 4, gl.FLOAT, false, VERTEX_BYTES, 12);
		// Only the beam direction is translucent; everything else is opaque and blends to itself.
		gl.enable(gl.BLEND);
		gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
		renderDepthMaskedLinework(
			gl,
			() => {
				gl.bufferData(
					gl.ARRAY_BUFFER,
					new Float32Array(painter.fillVertices),
					gl.DYNAMIC_DRAW,
				);
				gl.drawArrays(gl.TRIANGLES, 0, painter.fillVertices.length / VERTEX_FLOATS);
			},
			() => {
				gl.bufferData(
					gl.ARRAY_BUFFER,
					new Float32Array(painter.depthLineVertices),
					gl.DYNAMIC_DRAW,
				);
				gl.drawArrays(gl.LINES, 0, painter.depthLineVertices.length / VERTEX_FLOATS);
			},
		);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array(painter.lineVertices),
			gl.DYNAMIC_DRAW,
		);
		gl.drawArrays(gl.LINES, 0, painter.lineVertices.length / VERTEX_FLOATS);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array(painter.overlayVertices),
			gl.DYNAMIC_DRAW,
		);
		gl.drawArrays(gl.TRIANGLES, 0, painter.overlayVertices.length / VERTEX_FLOATS);
	}
}

export function observeViewportResize(
	element: Element,
	redraw: () => void,
): () => void {
	redraw();
	const observer = new ResizeObserver(() => redraw());
	observer.observe(element);
	return () => observer.disconnect();
}

function axisColor(axis: WorldAxis): LineColor {
	if (axis === "x") return [0.95, 0.16, 0.18];
	if (axis === "y") return [0.2, 0.78, 0.3];
	return [0.18, 0.46, 1];
}

/** A wide shaft from `origin` to the base of a filled head whose tip is `end`. */
function drawGizmoArrow(
	painter: Painter,
	origin: PlanPoint,
	end: PlanPoint,
	color: LineColor,
	head: number,
	width: number,
) {
	const dx = end[0] - origin[0];
	const dy = end[1] - origin[1];
	const length = Math.max(0.0001, Math.hypot(dx, dy));
	const unitX = dx / length;
	const unitY = dy / length;
	const back: PlanPoint = [end[0] - unitX * head * 1.8, end[1] - unitY * head * 1.8];
	painter.stroke(origin, back, color, width);
	painter.triangle(
		end,
		[back[0] - unitY * head, back[1] + unitX * head],
		[back[0] + unitY * head, back[1] - unitX * head],
		color,
	);
}

function dottedGuide(
	line: (a: [number, number], b: [number, number], color: LineColor) => void,
	origin: [number, number],
	horizontal: boolean,
	color: LineColor,
	camera: TileCamera,
	canvas: HTMLCanvasElement,
) {
	const [start, end] = viewportGuideRange(
		horizontal,
		camera,
		canvas.clientWidth,
		canvas.clientHeight,
	);
	const dash = 8 / camera.zoom;
	for (let cursor = start; cursor < end; cursor += dash * 2) {
		if (horizontal)
			line(
				[cursor, origin[1]],
				[Math.min(end, cursor + dash), origin[1]],
				color,
			);
		else
			line(
				[origin[0], cursor],
				[origin[0], Math.min(end, cursor + dash)],
				color,
			);
	}
}

function shader(gl: WebGL2RenderingContext, type: number, source: string) {
	const value = gl.createShader(type);
	if (!value) return null;
	gl.shaderSource(value, source);
	gl.compileShader(value);
	return gl.getShaderParameter(value, gl.COMPILE_STATUS) ? value : null;
}
