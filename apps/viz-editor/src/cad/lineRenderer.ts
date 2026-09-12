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
import { previewDeltaForEntity, projectPoint, viewAxes } from "./types";

export type LineColor = [number, number, number];

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
}

/** The three vertex arrays of a frame, and the closures that append plan points to them. */
interface Painter {
	fillVertices: number[];
	depthLineVertices: number[];
	lineVertices: number[];
	vertex(
		vertices: number[],
		point: PlanPoint,
		color: LineColor,
		depth?: number,
	): void;
	line(a: PlanPoint, b: PlanPoint, color: LineColor): void;
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
			...color,
		);
	};
	return {
		fillVertices,
		depthLineVertices,
		lineVertices,
		vertex,
		line: (a, b, color) => {
			vertex(lineVertices, a, color);
			vertex(lineVertices, b, color);
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

/** The datum guide of an elevation, and the coordinate arrows when they are switched on. */
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
	const axes = viewAxes(view, rotationQuarterTurns);
	const length = 54 / camera.zoom;
	const head = 5 / camera.zoom;
	drawArrow(
		painter.line,
		worldOrigin,
		[worldOrigin[0] + length * axes.horizontal.sign, worldOrigin[1]],
		axisColor(axes.horizontal.axis),
		head,
	);
	drawArrow(
		painter.line,
		worldOrigin,
		[worldOrigin[0], worldOrigin[1] + length * axes.vertical.sign],
		axisColor(axes.vertical.axis),
		head,
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
		const key = `${entity.drawingId}:${view}:${entity.sizeMillimetres.join(",")}:${entity.rotationDegrees.join(",")}`;
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
			const direction = projectPoint(
				entity.outputDirection.map((value) => value * 420) as [
					number,
					number,
					number,
				],
				view,
				rotationQuarterTurns,
			);
			painter.line(
				centre,
				[centre[0] + direction[0], centre[1] + direction[1]],
				outlineColor,
			);
		}
	}
}

/** The move gizmo beside the selection, and the axis guide a constrained drag follows. */
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
	painter.line(
		[origin[0] - square, origin[1] - square],
		[origin[0] + square, origin[1] - square],
		handle,
	);
	painter.line(
		[origin[0] + square, origin[1] - square],
		[origin[0] + square, origin[1] + square],
		handle,
	);
	painter.line(
		[origin[0] + square, origin[1] + square],
		[origin[0] - square, origin[1] + square],
		handle,
	);
	painter.line(
		[origin[0] - square, origin[1] + square],
		[origin[0] - square, origin[1] - square],
		handle,
	);
	drawArrow(painter.line, origin, [origin[0] + length, origin[1]], horizontal, square);
	drawArrow(painter.line, origin, [origin[0], origin[1] + length], vertical, square);
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
			in vec3 position; in vec3 color; out vec3 lineColor;
			void main(){ gl_Position=vec4(position,1.0); lineColor=color; }`,
		);
		const fragment = shader(
			gl,
			gl.FRAGMENT_SHADER,
			`#version 300 es
			precision mediump float; in vec3 lineColor; out vec4 outputColor;
			void main(){ outputColor=vec4(lineColor,1.0); }`,
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
		paintGizmo(painter, frame, this.canvas);
		paintSelectionBox(painter, frame);
		this.upload(painter);
	}

	private upload(painter: Painter) {
		const gl = this.gl;
		gl.useProgram(this.program);
		gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
		const position = gl.getAttribLocation(this.program, "position");
		const color = gl.getAttribLocation(this.program, "color");
		gl.enableVertexAttribArray(position);
		gl.vertexAttribPointer(position, 3, gl.FLOAT, false, 24, 0);
		gl.enableVertexAttribArray(color);
		gl.vertexAttribPointer(color, 3, gl.FLOAT, false, 24, 12);
		renderDepthMaskedLinework(
			gl,
			() => {
				gl.bufferData(
					gl.ARRAY_BUFFER,
					new Float32Array(painter.fillVertices),
					gl.DYNAMIC_DRAW,
				);
				gl.drawArrays(gl.TRIANGLES, 0, painter.fillVertices.length / 6);
			},
			() => {
				gl.bufferData(
					gl.ARRAY_BUFFER,
					new Float32Array(painter.depthLineVertices),
					gl.DYNAMIC_DRAW,
				);
				gl.drawArrays(gl.LINES, 0, painter.depthLineVertices.length / 6);
			},
		);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array(painter.lineVertices),
			gl.DYNAMIC_DRAW,
		);
		gl.drawArrays(gl.LINES, 0, painter.lineVertices.length / 6);
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

function drawArrow(
	line: (a: [number, number], b: [number, number], color: LineColor) => void,
	origin: [number, number],
	end: [number, number],
	color: LineColor,
	head: number,
) {
	line(origin, end, color);
	const dx = end[0] - origin[0];
	const dy = end[1] - origin[1];
	const length = Math.max(0.0001, Math.hypot(dx, dy));
	const unitX = dx / length;
	const unitY = dy / length;
	const back: [number, number] = [
		end[0] - unitX * head * 1.8,
		end[1] - unitY * head * 1.8,
	];
	line(end, [back[0] - unitY * head, back[1] + unitX * head], color);
	line(end, [back[0] + unitY * head, back[1] - unitX * head], color);
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
