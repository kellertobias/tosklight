import { useEffect, useMemo, useRef, useState } from "react";
import type {
	CadEntity,
	CadViewDirection,
	SelectionChange,
	TileCamera,
	WorldAxis,
} from "./types";
import { planeDelta, projectPoint, viewAxes } from "./types";

interface CadViewportProps {
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	snapToMounts: boolean;
	onCamera(camera: TileCamera): void;
	onSelection(change: SelectionChange): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
	): Promise<void>;
}

interface Drag {
	type: "pan" | "move" | "box";
	start: [number, number];
	last: [number, number];
	axis: "plane" | "horizontal" | "vertical";
	entityIds?: readonly string[];
	startCamera?: TileCamera;
	additive?: boolean;
}

interface SelectionBox {
	start: [number, number];
	end: [number, number];
}

type MoveAxis = "plane" | "horizontal" | "vertical";

export function CadViewport({
	entities,
	selectedIds,
	view,
	rotationQuarterTurns,
	camera,
	onCamera,
	onSelection,
	onMove,
}: CadViewportProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const drag = useRef<Drag | null>(null);
	const [preview, setPreview] = useState<[number, number]>([0, 0]);
	const [guide, setGuide] = useState<MoveAxis | null>(null);
	const [selectionBox, setSelectionBox] = useState<SelectionBox | null>(null);
	const selected = useMemo(() => new Set(selectedIds), [selectedIds]);

	useEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		const active = renderer.current;
		const resize = () => active?.resize();
		resize();
		const observer = new ResizeObserver(resize);
		observer.observe(canvas.current);
		return () => observer.disconnect();
	}, []);

	useEffect(() => {
		renderer.current?.draw(
			entities,
			selected,
			view,
			rotationQuarterTurns,
			camera,
			preview,
			guide,
			selectionBox,
		);
	}, [
		entities,
		selected,
		view,
		rotationQuarterTurns,
		camera,
		preview,
		guide,
		selectionBox,
	]);

	function screenToPlane(clientX: number, clientY: number): [number, number] {
		const bounds = canvas.current?.getBoundingClientRect();
		if (!bounds) return [0, 0];
		return [
			(clientX - bounds.left - bounds.width / 2) / camera.zoom - camera.pan[0],
			-(clientY - bounds.top - bounds.height / 2) / camera.zoom - camera.pan[1],
		];
	}

	function pick(clientX: number, clientY: number): CadEntity | null {
		const point = screenToPlane(clientX, clientY);
		let best: { entity: CadEntity; distance: number } | null = null;
		for (const entity of entities) {
			const projected = projectPoint(
				entity.positionMillimetres,
				view,
				rotationQuarterTurns,
			);
			const distance = Math.hypot(
				projected[0] - point[0],
				projected[1] - point[1],
			);
			const threshold = Math.max(
				180,
				Math.min(800, entity.sizeMillimetres[0] / 2),
			);
			if (distance <= threshold && (!best || distance < best.distance)) {
				best = { entity, distance };
			}
		}
		return best?.entity ?? null;
	}

	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
		canvas.current?.setPointerCapture(event.pointerId);
		const hit = pick(event.clientX, event.clientY);
		if (event.button === 1 || event.altKey) {
			drag.current = {
				type: "pan",
				start: [event.clientX, event.clientY],
				last: [event.clientX, event.clientY],
				axis: "plane",
				startCamera: camera,
			};
			return;
		}
		const point = screenToPlane(event.clientX, event.clientY);
		const axis = pickGizmo(
			point,
			entities,
			selected,
			view,
			rotationQuarterTurns,
			camera,
		);
		if (axis) {
			drag.current = {
				type: "move",
				start: [event.clientX, event.clientY],
				last: [event.clientX, event.clientY],
				axis,
				entityIds: selectedIds,
			};
			setGuide(axis);
			return;
		}
		if (hit) {
			onSelection({
				type: event.shiftKey ? "toggle" : "replace",
				ids: [hit.id],
			});
			return;
		}
		drag.current = {
			type: "box",
			start: [event.clientX, event.clientY],
			last: [event.clientX, event.clientY],
			axis: "plane",
			additive: event.shiftKey,
		};
		setSelectionBox({ start: point, end: point });
	}

	function pointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		if (!active) return;
		const dx = event.clientX - active.start[0];
		const dy = event.clientY - active.start[1];
		active.last = [event.clientX, event.clientY];
		if (active.type === "pan") {
			const start = active.startCamera ?? camera;
			onCamera({
				...start,
				pan: [start.pan[0] + dx / start.zoom, start.pan[1] - dy / start.zoom],
			});
			return;
		}
		if (active.type === "box") {
			setSelectionBox({
				start: screenToPlane(...active.start),
				end: screenToPlane(event.clientX, event.clientY),
			});
			return;
		}
		setPreview([
			active.axis === "vertical" ? 0 : dx / camera.zoom,
			active.axis === "horizontal" ? 0 : -dy / camera.zoom,
		]);
	}

	async function pointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		drag.current = null;
		canvas.current?.releasePointerCapture(event.pointerId);
		if (active?.type === "box") {
			const box = selectionBox;
			setSelectionBox(null);
			if (!box) return;
			const moved = Math.hypot(
				event.clientX - active.start[0],
				event.clientY - active.start[1],
			);
			const ids =
				moved < 3
					? []
					: entities
							.filter((entity) =>
								pointInside(
									projectPoint(
										entity.positionMillimetres,
										view,
										rotationQuarterTurns,
									),
									box,
								),
							)
							.map((entity) => entity.id);
			onSelection({ type: active.additive ? "add" : "replace", ids });
			return;
		}
		if (active?.type !== "move") return;
		const current = preview;
		setPreview([0, 0]);
		setGuide(null);
		if (Math.hypot(current[0], current[1]) < 1) return;
		await onMove(
			planeDelta(current, view, rotationQuarterTurns),
			active.entityIds ?? selectedIds,
		);
	}

	return (
		<canvas
			ref={canvas}
			className="cad-canvas"
			aria-label={`CAD ${view.replaceAll("_", " ")} viewport`}
			onPointerDown={pointerDown}
			onPointerMove={pointerMove}
			onPointerUp={pointerUp}
			onPointerCancel={() => {
				drag.current = null;
				setPreview([0, 0]);
				setGuide(null);
				setSelectionBox(null);
			}}
			onWheel={(event) => {
				event.preventDefault();
				onCamera({
					...camera,
					zoom: Math.min(
						2.5,
						Math.max(0.004, camera.zoom * Math.exp(-event.deltaY * 0.0015)),
					),
				});
			}}
		/>
	);
}

class LineRenderer {
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
			in vec2 position; in vec3 color; out vec3 lineColor;
			void main(){ gl_Position=vec4(position,0.0,1.0); lineColor=color; }`,
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

	draw(
		entities: readonly CadEntity[],
		selected: ReadonlySet<string>,
		view: CadViewDirection,
		rotationQuarterTurns: number,
		camera: TileCamera,
		preview: readonly [number, number],
		guide: MoveAxis | null,
		selectionBox: SelectionBox | null,
	) {
		this.resize();
		const gl = this.gl;
		gl.viewport(0, 0, this.canvas.width, this.canvas.height);
		gl.clearColor(0.018, 0.024, 0.032, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
		const vertices: number[] = [];
		const line = (
			a: [number, number],
			b: [number, number],
			color: [number, number, number],
		) => {
			for (const point of [a, b]) {
				vertices.push(
					((point[0] + camera.pan[0]) * camera.zoom * 2) /
						this.canvas.clientWidth,
					((point[1] + camera.pan[1]) * camera.zoom * 2) /
						this.canvas.clientHeight,
					...color,
				);
			}
		};
		for (const entity of entities) {
			const active = selected.has(entity.id);
			const centre = projectPoint(
				entity.positionMillimetres,
				view,
				rotationQuarterTurns,
			);
			if (active) {
				centre[0] += preview[0];
				centre[1] += preview[1];
			}
			const [width, height] = projectedSize(entity, view, rotationQuarterTurns);
			const halfX = Math.max(90, width / 2);
			const halfY = Math.max(90, height / 2);
			const color: [number, number, number] = active
				? [0.02, 0.82, 0.98]
				: entity.kind === "venue"
					? [0.56, 0.62, 0.68]
					: [0.92, 0.94, 0.97];
			const corners: [number, number][] = [
				[centre[0] - halfX, centre[1] - halfY],
				[centre[0] + halfX, centre[1] - halfY],
				[centre[0] + halfX, centre[1] + halfY],
				[centre[0] - halfX, centre[1] + halfY],
			];
			for (let index = 0; index < 4; index++)
				line(corners[index], corners[(index + 1) % 4], color);
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
				line(
					centre,
					[centre[0] + direction[0], centre[1] + direction[1]],
					color,
				);
			}
		}
		const gizmo = gizmoGeometry(
			entities,
			selected,
			view,
			rotationQuarterTurns,
			camera,
			preview,
		);
		if (gizmo) {
			const axes = viewAxes(view, rotationQuarterTurns);
			const horizontal = axisColor(axes.horizontal.axis);
			const vertical = axisColor(axes.vertical.axis);
			const { origin, length, square } = gizmo;
			line(
				[origin[0] - square, origin[1] - square],
				[origin[0] + square, origin[1] - square],
				[0.75, 0.8, 0.84],
			);
			line(
				[origin[0] + square, origin[1] - square],
				[origin[0] + square, origin[1] + square],
				[0.75, 0.8, 0.84],
			);
			line(
				[origin[0] + square, origin[1] + square],
				[origin[0] - square, origin[1] + square],
				[0.75, 0.8, 0.84],
			);
			line(
				[origin[0] - square, origin[1] + square],
				[origin[0] - square, origin[1] - square],
				[0.75, 0.8, 0.84],
			);
			drawArrow(
				line,
				origin,
				[origin[0] + length, origin[1]],
				horizontal,
				square,
			);
			drawArrow(
				line,
				origin,
				[origin[0], origin[1] + length],
				vertical,
				square,
			);
			if (guide === "horizontal")
				dottedGuide(line, origin, true, horizontal, camera, this.canvas);
			if (guide === "vertical")
				dottedGuide(line, origin, false, vertical, camera, this.canvas);
		}
		if (selectionBox) {
			const { start, end } = selectionBox;
			const color: [number, number, number] = [0.02, 0.82, 0.98];
			line([start[0], start[1]], [end[0], start[1]], color);
			line([end[0], start[1]], [end[0], end[1]], color);
			line([end[0], end[1]], [start[0], end[1]], color);
			line([start[0], end[1]], [start[0], start[1]], color);
		}
		gl.useProgram(this.program);
		gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
		const position = gl.getAttribLocation(this.program, "position");
		const color = gl.getAttribLocation(this.program, "color");
		gl.enableVertexAttribArray(position);
		gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 20, 0);
		gl.enableVertexAttribArray(color);
		gl.vertexAttribPointer(color, 3, gl.FLOAT, false, 20, 8);
		gl.drawArrays(gl.LINES, 0, vertices.length / 5);
	}
}

function pointInside(point: [number, number], box: SelectionBox): boolean {
	return (
		point[0] >= Math.min(box.start[0], box.end[0]) &&
		point[0] <= Math.max(box.start[0], box.end[0]) &&
		point[1] >= Math.min(box.start[1], box.end[1]) &&
		point[1] <= Math.max(box.start[1], box.end[1])
	);
}

function gizmoGeometry(
	entities: readonly CadEntity[],
	selected: ReadonlySet<string>,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	camera: TileCamera,
	preview: readonly [number, number] = [0, 0],
) {
	const points = entities
		.filter((entity) => selected.has(entity.id))
		.map((entity) =>
			projectPoint(entity.positionMillimetres, view, rotationQuarterTurns),
		);
	if (!points.length) return null;
	const centre: [number, number] = [
		points.reduce((sum, point) => sum + point[0], 0) / points.length +
			preview[0],
		points.reduce((sum, point) => sum + point[1], 0) / points.length +
			preview[1],
	];
	return {
		origin: [centre[0] + 36 / camera.zoom, centre[1] + 36 / camera.zoom] as [
			number,
			number,
		],
		length: 48 / camera.zoom,
		square: 7 / camera.zoom,
	};
}

function pickGizmo(
	point: [number, number],
	entities: readonly CadEntity[],
	selected: ReadonlySet<string>,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	camera: TileCamera,
): MoveAxis | null {
	const gizmo = gizmoGeometry(
		entities,
		selected,
		view,
		rotationQuarterTurns,
		camera,
	);
	if (!gizmo) return null;
	const { origin, length } = gizmo;
	const tolerance = 10 / camera.zoom;
	if (Math.hypot(point[0] - origin[0], point[1] - origin[1]) <= tolerance)
		return "plane";
	if (
		point[0] >= origin[0] &&
		point[0] <= origin[0] + length &&
		Math.abs(point[1] - origin[1]) <= tolerance
	)
		return "horizontal";
	if (
		point[1] >= origin[1] &&
		point[1] <= origin[1] + length &&
		Math.abs(point[0] - origin[0]) <= tolerance
	)
		return "vertical";
	return null;
}

function axisColor(axis: WorldAxis): [number, number, number] {
	if (axis === "x") return [0.95, 0.16, 0.18];
	if (axis === "y") return [0.2, 0.78, 0.3];
	return [0.18, 0.46, 1];
}

function drawArrow(
	line: (
		a: [number, number],
		b: [number, number],
		color: [number, number, number],
	) => void,
	origin: [number, number],
	end: [number, number],
	color: [number, number, number],
	head: number,
) {
	line(origin, end, color);
	if (end[1] === origin[1]) {
		line(end, [end[0] - head * 1.8, end[1] - head], color);
		line(end, [end[0] - head * 1.8, end[1] + head], color);
	} else {
		line(end, [end[0] - head, end[1] - head * 1.8], color);
		line(end, [end[0] + head, end[1] - head * 1.8], color);
	}
}

function dottedGuide(
	line: (
		a: [number, number],
		b: [number, number],
		color: [number, number, number],
	) => void,
	origin: [number, number],
	horizontal: boolean,
	color: [number, number, number],
	camera: TileCamera,
	canvas: HTMLCanvasElement,
) {
	const extent =
		(horizontal ? canvas.clientWidth : canvas.clientHeight) / camera.zoom;
	const start = (horizontal ? origin[0] : origin[1]) - extent;
	const end = (horizontal ? origin[0] : origin[1]) + extent;
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

function projectedSize(
	entity: CadEntity,
	view: CadViewDirection,
	rotationQuarterTurns: number,
): [number, number] {
	const [width, depth, height] = entity.sizeMillimetres;
	switch (view) {
		case "top_down":
			return Math.abs(rotationQuarterTurns) % 2 === 1
				? [depth, width]
				: [width, depth];
		case "left_to_right":
		case "right_to_left":
			return [depth, height];
		case "front_to_back":
		case "back_to_front":
			return [width, height];
	}
}
