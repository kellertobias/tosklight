import { useEffect, useMemo, useRef, useState } from "react";
import type {
	CadEntity,
	CadViewDirection,
	TileCamera,
} from "./types";
import { planeDelta, projectPoint } from "./types";

interface CadViewportProps {
	entities: readonly CadEntity[];
	selectedIds: readonly string[];
	view: CadViewDirection;
	camera: TileCamera;
	snapToMounts: boolean;
	onCamera(camera: TileCamera): void;
	onSelection(ids: readonly string[]): void;
	onMove(deltaMillimetres: [number, number, number], entityIds: readonly string[]): Promise<void>;
}

interface Drag {
	type: "pan" | "move";
	start: [number, number];
	last: [number, number];
	axis: "plane" | "horizontal" | "vertical";
	entityIds?: readonly string[];
	startCamera?: TileCamera;
}

export function CadViewport({
	entities,
	selectedIds,
	view,
	camera,
	onCamera,
	onSelection,
	onMove,
}: CadViewportProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const drag = useRef<Drag | null>(null);
	const [preview, setPreview] = useState<[number, number]>([0, 0]);
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
		renderer.current?.draw(entities, selected, view, camera, preview);
	}, [entities, selected, view, camera, preview]);

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
			const projected = projectPoint(entity.positionMillimetres, view);
			const distance = Math.hypot(projected[0] - point[0], projected[1] - point[1]);
			const threshold = Math.max(180, Math.min(800, entity.sizeMillimetres[0] / 2));
			if (distance <= threshold && (!best || distance < best.distance)) {
				best = { entity, distance };
			}
		}
		return best?.entity ?? null;
	}

	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
		canvas.current?.setPointerCapture(event.pointerId);
		const hit = pick(event.clientX, event.clientY);
		if (event.button === 1 || event.altKey || !hit) {
			drag.current = {
				type: "pan",
				start: [event.clientX, event.clientY],
				last: [event.clientX, event.clientY],
				axis: "plane",
				startCamera: camera,
			};
			if (!hit && event.button === 0 && !event.altKey) onSelection([]);
			return;
		}
		const next = event.shiftKey
			? selected.has(hit.id)
				? selectedIds.filter((id) => id !== hit.id)
				: [...selectedIds, hit.id]
			: selected.has(hit.id)
				? selectedIds
				: [hit.id];
		onSelection(next);
		const point = screenToPlane(event.clientX, event.clientY);
		const centre = projectPoint(hit.positionMillimetres, view);
		const dx = Math.abs(point[0] - centre[0]);
		const dy = Math.abs(point[1] - centre[1]);
		const axis = Math.hypot(dx, dy) < 160
			? "plane"
			: Math.abs(dy) < 100 && dx < 800
				? "horizontal"
				: Math.abs(dx) < 100 && dy < 800
					? "vertical"
					: "plane";
		drag.current = {
			type: "move",
			start: [event.clientX, event.clientY],
			last: [event.clientX, event.clientY],
			axis,
			entityIds: next,
		};
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
		setPreview([
			active.axis === "vertical" ? 0 : dx / camera.zoom,
			active.axis === "horizontal" ? 0 : -dy / camera.zoom,
		]);
	}

	async function pointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		drag.current = null;
		canvas.current?.releasePointerCapture(event.pointerId);
		if (active?.type !== "move") return;
		const current = preview;
		setPreview([0, 0]);
		if (Math.hypot(current[0], current[1]) < 1) return;
		await onMove(planeDelta(current, view), active.entityIds ?? selectedIds);
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
			}}
			onWheel={(event) => {
				event.preventDefault();
				onCamera({
					...camera,
					zoom: Math.min(2.5, Math.max(0.004, camera.zoom * Math.exp(-event.deltaY * 0.0015))),
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
		camera: TileCamera,
		preview: readonly [number, number],
	) {
		this.resize();
		const { gl } = this;
		gl.viewport(0, 0, this.canvas.width, this.canvas.height);
		gl.clearColor(0.018, 0.024, 0.032, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
		const vertices: number[] = [];
		const line = (a: [number, number], b: [number, number], color: [number, number, number]) => {
			for (const point of [a, b]) {
				vertices.push(
					(point[0] + camera.pan[0]) * camera.zoom * 2 / this.canvas.clientWidth,
					(point[1] + camera.pan[1]) * camera.zoom * 2 / this.canvas.clientHeight,
					...color,
				);
			}
		};
		for (const entity of entities) {
			const active = selected.has(entity.id);
			const centre = projectPoint(entity.positionMillimetres, view);
			if (active) {
				centre[0] += preview[0];
				centre[1] += preview[1];
			}
			const [width, height] = projectedSize(entity, view);
			const halfX = Math.max(90, width / 2);
			const halfY = Math.max(90, height / 2);
			const color: [number, number, number] = active
				? [1, 0.12, 0.12]
				: entity.kind === "venue"
					? [0.56, 0.62, 0.68]
					: [0.92, 0.94, 0.97];
			const corners: [number, number][] = [
				[centre[0] - halfX, centre[1] - halfY],
				[centre[0] + halfX, centre[1] - halfY],
				[centre[0] + halfX, centre[1] + halfY],
				[centre[0] - halfX, centre[1] + halfY],
			];
			for (let index = 0; index < 4; index++) line(corners[index], corners[(index + 1) % 4], color);
			if (entity.kind !== "venue") {
				const direction = projectPoint(entity.outputDirection.map((value) => value * 420) as [number, number, number], view);
				line(centre, [centre[0] + direction[0], centre[1] + direction[1]], color);
			}
			if (active) {
				line(centre, [centre[0] + 650, centre[1]], [1, 0.18, 0.18]);
				line(centre, [centre[0], centre[1] + 650], [1, 0.18, 0.18]);
			}
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

function shader(gl: WebGL2RenderingContext, type: number, source: string) {
	const value = gl.createShader(type);
	if (!value) return null;
	gl.shaderSource(value, source);
	gl.compileShader(value);
	return gl.getShaderParameter(value, gl.COMPILE_STATUS) ? value : null;
}

function projectedSize(entity: CadEntity, view: CadViewDirection): [number, number] {
	const [width, depth, height] = entity.sizeMillimetres;
	switch (view) {
		case "top_down": return [width, depth];
		case "left_to_right":
		case "right_to_left": return [depth, height];
		case "front_to_back":
		case "back_to_front": return [width, height];
	}
}
