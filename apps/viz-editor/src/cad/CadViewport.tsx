import { useEffect, useMemo, useRef, useState } from "react";
import {
	entityPlanGeometry,
	type PlanGeometry,
	type PlanPoint,
} from "./projection";
import type {
	CadDrawing,
	CadEntity,
	CadPrintPage,
	CadTransformPreview,
	CadViewDirection,
	SelectionChange,
	TileCamera,
	WorldAxis,
} from "./types";
import { planeDelta, printPageHeight, projectPoint, viewAxes } from "./types";

interface CadViewportProps {
	entities: readonly CadEntity[];
	drawings: readonly CadDrawing[];
	selectedIds: readonly string[];
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	preview: CadTransformPreview | null;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
	editEnabled?: boolean;
	printPages?: readonly CadPrintPage[];
	selectedPrintPageId?: string | null;
	onSelectPrintPage?(id: string): void;
	onChangePrintPage?(id: string, change: Partial<CadPrintPage>): void;
	onCamera(camera: TileCamera): void;
	onSelection(change: SelectionChange): void;
	onPreview(preview: CadTransformPreview | null): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
	): Promise<void>;
}

interface CadRigOverviewProps {
	entities: readonly CadEntity[];
	drawings: readonly CadDrawing[];
	showName: string;
}

const OVERVIEW_VIEW: CadViewDirection = "top_down";
const OVERVIEW_ROTATION_QUARTER_TURNS = -1;

/** A fixed, read-only Show-screen plan rendered by the canonical CAD drawing pipeline. */
export function CadRigOverview({
	entities,
	drawings,
	showName,
}: CadRigOverviewProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const redraw = useRef<() => void>(() => undefined);
	const drawingById = useMemo(
		() => new Map(drawings.map((drawing) => [drawing.id, drawing])),
		[drawings],
	);

	useEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		return observeViewportResize(canvas.current, () => redraw.current());
	}, []);

	useEffect(() => {
		redraw.current = () => {
			const target = canvas.current;
			if (!target) return;
			const camera = fitCadOverview(
				entities,
				drawingById,
				target.clientWidth,
				target.clientHeight,
			);
			renderer.current?.draw(
				entities,
				drawingById,
				new Set(),
				OVERVIEW_VIEW,
				OVERVIEW_ROTATION_QUARTER_TURNS,
				camera,
				null,
				false,
				null,
				null,
			);
		};
		redraw.current();
	}, [entities, drawingById]);

	return (
		<canvas
			ref={canvas}
			className="cad-canvas viz-show-rig-canvas"
			role="img"
			aria-label={`Read-only rig overview for ${showName}`}
			data-view={OVERVIEW_VIEW}
			data-rotation-quarter-turns={OVERVIEW_ROTATION_QUARTER_TURNS}
			data-entity-count={entities.length}
		/>
	);
}

interface Drag {
	type: "pan" | "move" | "box";
	start: [number, number];
	last: [number, number];
	axis: "plane" | "horizontal" | "vertical";
	entityIds?: readonly string[];
	startCamera?: TileCamera;
	additive?: boolean;
	deltaMillimetres?: [number, number, number];
}

interface SelectionBox {
	start: [number, number];
	end: [number, number];
}

type MoveAxis = "plane" | "horizontal" | "vertical";

export function CadViewport({
	entities,
	drawings,
	selectedIds,
	view,
	rotationQuarterTurns,
	camera,
	preview,
	showFixtureIds,
	showDmxAddresses,
	editEnabled = true,
	printPages = [],
	selectedPrintPageId = null,
	onSelectPrintPage,
	onChangePrintPage,
	onCamera,
	onSelection,
	onPreview,
	onMove,
}: CadViewportProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const redraw = useRef<() => void>(() => undefined);
	const drag = useRef<Drag | null>(null);
	const [guide, setGuide] = useState<MoveAxis | null>(null);
	const [selectionBox, setSelectionBox] = useState<SelectionBox | null>(null);
	const selected = useMemo(() => new Set(selectedIds), [selectedIds]);
	const drawingById = useMemo(
		() => new Map(drawings.map((drawing) => [drawing.id, drawing])),
		[drawings],
	);

	useEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		return observeViewportResize(canvas.current, () => redraw.current());
	}, []);

	useEffect(() => {
		redraw.current = () =>
			renderer.current?.draw(
				entities,
				drawingById,
				selected,
				view,
				rotationQuarterTurns,
				camera,
				preview,
				editEnabled,
				guide,
				selectionBox,
			);
		redraw.current();
	}, [
		entities,
		drawingById,
		selected,
		view,
		rotationQuarterTurns,
		camera,
		preview,
		editEnabled,
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
		const ordered = [...entities].sort(
			(left, right) => viewDepth(right, view) - viewDepth(left, view),
		);
		let best: { entity: CadEntity; distance: number } | null = null;
		for (const entity of ordered) {
			if (!entity.selectable) continue;
			const projected = projectPoint(
				entity.positionMillimetres,
				view,
				rotationQuarterTurns,
			);
			const geometry = worldGeometry(
				entity,
				entityPlanGeometry(entity, drawingById.get(entity.drawingId), view),
				view,
				rotationQuarterTurns,
			);
			if (
				geometry.triangles.some((triangle) =>
					pointInTriangle(point, triangle.points),
				)
			)
				return entity;
			const distance = Math.hypot(
				projected[0] - point[0],
				projected[1] - point[1],
			);
			const threshold = Math.max(90, 8 / camera.zoom);
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
		if (!editEnabled) return;
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
				entityIds: entities
					.filter((entity) => entity.selectable && selected.has(entity.id))
					.map((entity) => entity.id),
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
		const localDelta: [number, number] = [
			active.axis === "vertical" ? 0 : dx / camera.zoom,
			active.axis === "horizontal" ? 0 : -dy / camera.zoom,
		];
		const deltaMillimetres = planeDelta(localDelta, view, rotationQuarterTurns);
		active.deltaMillimetres = deltaMillimetres;
		onPreview({
			entityIds: active.entityIds ?? selectedIds,
			deltaMillimetres,
		});
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
							.filter((entity) => entity.selectable)
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
		const current = active.deltaMillimetres ?? [0, 0, 0];
		onPreview(null);
		setGuide(null);
		if (Math.hypot(...current) < 1) return;
		await onMove(current, active.entityIds ?? selectedIds);
	}

	return (
		<div className="cad-viewport">
			<canvas
				ref={canvas}
				className="cad-canvas"
				aria-label={`CAD ${view.replaceAll("_", " ")} viewport`}
				onPointerDown={pointerDown}
				onPointerMove={pointerMove}
				onPointerUp={pointerUp}
				onPointerCancel={() => {
					drag.current = null;
					onPreview(null);
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
			{showFixtureIds || showDmxAddresses ? (
				<div className="cad-entity-labels" aria-hidden="true">
					{entities.map((entity) => {
						const worldDelta = preview?.entityIds.includes(entity.id)
							? preview.deltaMillimetres
							: ([0, 0, 0] as const);
						const point = projectPoint(
							entity.positionMillimetres.map(
								(value, index) => value + worldDelta[index],
							) as [number, number, number],
							view,
							rotationQuarterTurns,
						);
						return (
							<span
								key={entity.id}
								className="cad-entity-label"
								style={{
									left: `calc(50% + ${(point[0] + camera.pan[0]) * camera.zoom}px)`,
									top: `calc(50% - ${(point[1] + camera.pan[1]) * camera.zoom}px)`,
								}}
							>
								{showFixtureIds ? `ID ${entity.fixtureDisplayId}` : null}
								{showFixtureIds && showDmxAddresses ? " · " : null}
								{showDmxAddresses ? `DMX ${entity.dmxAddress}` : null}
							</span>
						);
					})}
				</div>
			) : null}
			{printPages.length ? (
				<div className="cad-print-frames">
					{printPages.map((page) => (
						<PrintFrame
							key={page.id}
							page={page}
							camera={camera}
							selected={page.id === selectedPrintPageId}
							onSelect={() => onSelectPrintPage?.(page.id)}
							onChange={(change) => onChangePrintPage?.(page.id, change)}
						/>
					))}
				</div>
			) : null}
		</div>
	);
}

function PrintFrame({
	page,
	camera,
	selected,
	onSelect,
	onChange,
}: {
	page: CadPrintPage;
	camera: TileCamera;
	selected: boolean;
	onSelect(): void;
	onChange(change: Partial<CadPrintPage>): void;
}) {
	const interaction = useRef<{
		type: "move" | "scale";
		start: [number, number];
		centre: [number, number];
		width: number;
	} | null>(null);
	const height = printPageHeight(page);
	function move(event: React.PointerEvent<HTMLElement>) {
		const active = interaction.current;
		if (!active) return;
		const dx = event.clientX - active.start[0];
		const dy = event.clientY - active.start[1];
		if (active.type === "move") {
			onChange({
				centreMillimetres: [
					active.centre[0] + dx / camera.zoom,
					active.centre[1] - dy / camera.zoom,
				],
			});
		} else {
			onChange({
				widthMillimetres: Math.max(
					500,
					active.width +
						Math.max(
							(dx * 2) / camera.zoom,
							(dy * 2 * 297) / (camera.zoom * 210),
						),
				),
			});
		}
	}
	function stop(event: React.PointerEvent<HTMLElement>) {
		interaction.current = null;
		event.currentTarget.releasePointerCapture?.(event.pointerId);
	}
	return (
		<div
			className={`cad-print-frame ${selected ? "is-selected" : ""}`}
			style={{
				left: `calc(50% + ${(page.centreMillimetres[0] - page.widthMillimetres / 2 + camera.pan[0]) * camera.zoom}px)`,
				top: `calc(50% - ${(page.centreMillimetres[1] + height / 2 + camera.pan[1]) * camera.zoom}px)`,
				width: `${page.widthMillimetres * camera.zoom}px`,
				height: `${height * camera.zoom}px`,
			}}
			onPointerDown={(event) => {
				event.preventDefault();
				event.stopPropagation();
				onSelect();
				interaction.current = {
					type: "move",
					start: [event.clientX, event.clientY],
					centre: page.centreMillimetres,
					width: page.widthMillimetres,
				};
				event.currentTarget.setPointerCapture?.(event.pointerId);
			}}
			onPointerMove={move}
			onPointerUp={stop}
			onPointerCancel={stop}
		>
			<span>{page.name}</span>
			<button
				type="button"
				className="cad-print-scale"
				aria-label={`Scale ${page.name}`}
				onPointerDown={(event) => {
					event.preventDefault();
					event.stopPropagation();
					onSelect();
					interaction.current = {
						type: "scale",
						start: [event.clientX, event.clientY],
						centre: page.centreMillimetres,
						width: page.widthMillimetres,
					};
					event.currentTarget.setPointerCapture?.(event.pointerId);
				}}
				onPointerMove={move}
				onPointerUp={stop}
				onPointerCancel={stop}
			/>
		</div>
	);
}

class LineRenderer {
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
		drawings: ReadonlyMap<string, CadDrawing>,
		selected: ReadonlySet<string>,
		view: CadViewDirection,
		rotationQuarterTurns: number,
		camera: TileCamera,
		preview: CadTransformPreview | null,
		editEnabled: boolean,
		guide: MoveAxis | null,
		selectionBox: SelectionBox | null,
	) {
		this.resize();
		const gl = this.gl;
		gl.viewport(0, 0, this.canvas.width, this.canvas.height);
		gl.clearColor(0.018, 0.024, 0.032, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
		const fillVertices: number[] = [];
		const lineVertices: number[] = [];
		const vertex = (
			vertices: number[],
			point: PlanPoint,
			color: [number, number, number],
		) => {
			vertices.push(
				((point[0] + camera.pan[0]) * camera.zoom * 2) /
					this.canvas.clientWidth,
				((point[1] + camera.pan[1]) * camera.zoom * 2) /
					this.canvas.clientHeight,
				...color,
			);
		};
		const line = (
			a: [number, number],
			b: [number, number],
			color: [number, number, number],
		) => {
			vertex(lineVertices, a, color);
			vertex(lineVertices, b, color);
		};
		const ordered = [...entities].sort(
			(left, right) => viewDepth(left, view) - viewDepth(right, view),
		);
		for (const entity of ordered) {
			const active = selected.has(entity.id);
			const entityPreview = preview?.entityIds.includes(entity.id)
				? projectPoint(preview.deltaMillimetres, view, rotationQuarterTurns)
				: ([0, 0] as const);
			const drawing = drawings.get(entity.drawingId);
			const key = `${entity.drawingId}:${view}:${entity.sizeMillimetres.join(",")}`;
			let geometry = this.geometryCache.get(key);
			if (!geometry) {
				geometry = entityPlanGeometry(entity, drawing, view);
				this.geometryCache.set(key, geometry);
			}
			const projected = worldGeometry(
				entity,
				geometry,
				view,
				rotationQuarterTurns,
				entityPreview,
			);
			for (const triangle of projected.triangles) {
				const color = active ? selectedColor(triangle.color) : triangle.color;
				for (const point of triangle.points) vertex(fillVertices, point, color);
			}
			const outlineColor: [number, number, number] = active
				? [0.02, 0.82, 0.98]
				: entity.kind === "venue"
					? [0.56, 0.62, 0.68]
					: [0.8, 0.84, 0.88];
			for (const outline of projected.outlines) {
				for (let index = 0; index < outline.length; index++) {
					line(
						outline[index],
						outline[(index + 1) % outline.length],
						outlineColor,
					);
				}
			}
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
				line(
					centre,
					[centre[0] + direction[0], centre[1] + direction[1]],
					outlineColor,
				);
			}
		}
		const gizmo = editEnabled
			? gizmoGeometry(
					entities,
					selected,
					view,
					rotationQuarterTurns,
					camera,
					preview
						? projectPoint(preview.deltaMillimetres, view, rotationQuarterTurns)
						: [0, 0],
				)
			: null;
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
		const position = gl.getAttribLocation(this.program, "position");
		const color = gl.getAttribLocation(this.program, "color");
		gl.enableVertexAttribArray(position);
		gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 20, 0);
		gl.enableVertexAttribArray(color);
		gl.vertexAttribPointer(color, 3, gl.FLOAT, false, 20, 8);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array(fillVertices),
			gl.DYNAMIC_DRAW,
		);
		gl.drawArrays(gl.TRIANGLES, 0, fillVertices.length / 5);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array(lineVertices),
			gl.DYNAMIC_DRAW,
		);
		gl.drawArrays(gl.LINES, 0, lineVertices.length / 5);
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

export function fitCadOverview(
	entities: readonly CadEntity[],
	drawings: ReadonlyMap<string, CadDrawing>,
	viewportWidth: number,
	viewportHeight: number,
): TileCamera {
	if (!entities.length) return { pan: [0, 0], zoom: 0.08 };
	const points: PlanPoint[] = [];
	for (const entity of entities) {
		const geometry = worldGeometry(
			entity,
			entityPlanGeometry(entity, drawings.get(entity.drawingId), OVERVIEW_VIEW),
			OVERVIEW_VIEW,
			OVERVIEW_ROTATION_QUARTER_TURNS,
		);
		for (const triangle of geometry.triangles) points.push(...triangle.points);
		for (const outline of geometry.outlines) points.push(...outline);
	}
	if (!points.length) {
		points.push(
			...entities.map((entity) =>
				projectPoint(
					entity.positionMillimetres,
					OVERVIEW_VIEW,
					OVERVIEW_ROTATION_QUARTER_TURNS,
				),
			),
		);
	}
	const minX = Math.min(...points.map((point) => point[0]));
	const maxX = Math.max(...points.map((point) => point[0]));
	const minY = Math.min(...points.map((point) => point[1]));
	const maxY = Math.max(...points.map((point) => point[1]));
	const width = Math.max(500, maxX - minX);
	const height = Math.max(500, maxY - minY);
	return {
		pan: [-(minX + maxX) / 2, -(minY + maxY) / 2],
		zoom: Math.max(
			0.001,
			Math.min(
				2.5,
				(Math.max(1, viewportWidth) * 0.88) / width,
				(Math.max(1, viewportHeight) * 0.82) / height,
			),
		),
	};
}

function worldGeometry(
	entity: CadEntity,
	geometry: PlanGeometry,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	offset: readonly [number, number] = [0, 0],
): PlanGeometry {
	const centre = projectPoint(
		entity.positionMillimetres,
		view,
		rotationQuarterTurns,
	);
	const angle =
		view === "top_down"
			? ((-entity.rotationDegrees[2] + rotationQuarterTurns * 90) * Math.PI) /
				180
			: 0;
	const cosine = Math.cos(angle);
	const sine = Math.sin(angle);
	const transform = (point: PlanPoint): PlanPoint => [
		centre[0] + point[0] * cosine - point[1] * sine + offset[0],
		centre[1] + point[0] * sine + point[1] * cosine + offset[1],
	];
	return {
		...geometry,
		triangles: geometry.triangles.map((triangle) => ({
			...triangle,
			points: triangle.points.map(transform) as [
				PlanPoint,
				PlanPoint,
				PlanPoint,
			],
		})),
		outlines: geometry.outlines.map((outline) => outline.map(transform)),
	};
}

function pointInTriangle(
	point: PlanPoint,
	triangle: [PlanPoint, PlanPoint, PlanPoint],
): boolean {
	const [a, b, c] = triangle;
	const area = (first: PlanPoint, second: PlanPoint, third: PlanPoint) =>
		(first[0] - third[0]) * (second[1] - third[1]) -
		(second[0] - third[0]) * (first[1] - third[1]);
	const total = area(a, b, c);
	if (Math.abs(total) < 0.0001) return false;
	const first = area(point, b, c) / total;
	const second = area(a, point, c) / total;
	const third = 1 - first - second;
	return first >= 0 && second >= 0 && third >= 0;
}

function selectedColor(
	color: readonly [number, number, number],
): [number, number, number] {
	const lightness = (color[0] + color[1] + color[2]) / 3;
	return [0.01, 0.45 + lightness * 0.42, 0.58 + lightness * 0.36];
}

function viewDepth(entity: CadEntity, view: CadViewDirection): number {
	switch (view) {
		case "top_down":
			return entity.positionMillimetres[2];
		case "left_to_right":
			return -entity.positionMillimetres[0];
		case "right_to_left":
			return entity.positionMillimetres[0];
		case "front_to_back":
			return -entity.positionMillimetres[1];
		case "back_to_front":
			return entity.positionMillimetres[1];
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
		.filter((entity) => entity.selectable && selected.has(entity.id))
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
