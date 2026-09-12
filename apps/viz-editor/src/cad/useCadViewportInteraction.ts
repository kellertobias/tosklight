/**
 * The pointer on a CAD viewport: panning, picking, the move gizmo and the marquee.
 *
 * The drag in flight lives in a ref rather than in state, because every pointer move would
 * otherwise re-render the whole viewport while the operator is still dragging. What the renderer
 * needs to draw — the axis guide and the marquee rectangle — is state, and nothing else is.
 */
import { useEffect, useRef, useState } from "react";
import type { SelectionBox } from "./lineRenderer";
import {
	boundsOf,
	entityBounds,
	marqueeCatches,
	marqueeMode,
} from "./marqueeSelection";
import { type MoveAxis, pickEntity, pickGizmo } from "./planGeometry";
import type {
	CadDrawing,
	CadEntity,
	CadTransformPreview,
	CadViewDirection,
	SelectionChange,
	TileCamera,
} from "./types";
import { planeDelta } from "./types";

interface Drag {
	type: "pan" | "move" | "box";
	start: [number, number];
	last: [number, number];
	axis: MoveAxis;
	entityIds?: readonly string[];
	startCamera?: TileCamera;
	additive?: boolean;
	deltaMillimetres?: [number, number, number];
	spread?: boolean;
	hitId?: string;
	marquee?: boolean;
}

export interface CadViewportInteraction {
	guide: MoveAxis | null;
	selectionBox: SelectionBox | null;
	pointerDown(event: React.PointerEvent<HTMLCanvasElement>): void;
	pointerMove(event: React.PointerEvent<HTMLCanvasElement>): void;
	pointerUp(event: React.PointerEvent<HTMLCanvasElement>): Promise<void>;
	/** Abandon a drag the pointer left behind, leaving the rig where it started. */
	cancel(): void;
}

/** Everything a gesture needs to read the viewport and report what it did. */
export interface CadViewportContext {
	canvas: React.RefObject<HTMLCanvasElement | null>;
	entities: readonly CadEntity[];
	drawingById: ReadonlyMap<string, CadDrawing>;
	selected: ReadonlySet<string>;
	selectedIds: readonly string[];
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	editEnabled: boolean;
	onCamera(camera: TileCamera): void;
	onSelection(change: SelectionChange): void;
	onPreview(preview: CadTransformPreview | null): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
	): Promise<void>;
}

/** Screen pixels to plan millimetres, through the tile's own camera. */
function screenToPlane(
	context: CadViewportContext,
	clientX: number,
	clientY: number,
): [number, number] {
	const { camera } = context;
	const bounds = context.canvas.current?.getBoundingClientRect();
	if (!bounds) return [0, 0];
	return [
		(clientX - bounds.left - bounds.width / 2) / camera.zoom - camera.pan[0],
		-(clientY - bounds.top - bounds.height / 2) / camera.zoom - camera.pan[1],
	];
}

/** What a drag of the gizmo has moved the selection by, and the preview that shows it. */
function updateMovePreview(
	context: CadViewportContext,
	active: Drag,
	clientX: number,
	clientY: number,
	spread: boolean,
) {
	const { camera, view, rotationQuarterTurns, selectedIds, onPreview } = context;
	const dx = clientX - active.start[0];
	const dy = clientY - active.start[1];
	const localDelta: [number, number] = [
		active.axis === "vertical" ? 0 : dx / camera.zoom,
		active.axis === "horizontal" ? 0 : -dy / camera.zoom,
	];
	const deltaMillimetres = planeDelta(localDelta, view, rotationQuarterTurns);
	active.deltaMillimetres = deltaMillimetres;
	active.spread = active.axis !== "plane" && spread;
	onPreview({
		entityIds: active.entityIds ?? selectedIds,
		deltaMillimetres,
		spread: active.spread,
	});
}

/** The drag a press starts: panning, a gizmo move, or a selection box. */
function beginDrag(
	context: CadViewportContext,
	event: React.PointerEvent<HTMLCanvasElement>,
	setGuide: (axis: MoveAxis | null) => void,
): Drag | null {
	const { camera, entities, selected, selectedIds, view, rotationQuarterTurns } =
		context;
	const hit = pickEntity(
		screenToPlane(context, event.clientX, event.clientY),
		entities,
		context.drawingById,
		view,
		rotationQuarterTurns,
		camera,
	);
	const start: [number, number] = [event.clientX, event.clientY];
	if (event.button === 1 || event.altKey)
		return {
			type: "pan",
			start,
			last: start,
			axis: "plane",
			startCamera: camera,
		};
	if (!context.editEnabled) return null;
	const axis = pickGizmo(
		screenToPlane(context, event.clientX, event.clientY),
		entities,
		selected,
		view,
		rotationQuarterTurns,
		camera,
	);
	if (axis) {
		const selectable = new Set(
			entities
				.filter((entity) => entity.selectable)
				.map((entity) => entity.logicalFixtureId),
		);
		setGuide(axis);
		return {
			type: "move",
			start,
			last: start,
			axis,
			entityIds: selectedIds.filter((id) => selectable.has(id)),
			spread: axis !== "plane" && event.shiftKey,
		};
	}
	return {
		type: "box",
		start,
		last: start,
		axis: "plane",
		additive: event.shiftKey,
		hitId: hit?.logicalFixtureId,
		marquee: false,
	};
}

/** What a finished marquee selects: the entities it caught, as the operator drew it on screen. */
function marqueeSelection(
	context: CadViewportContext,
	active: Drag,
	clientX: number,
): string[] {
	const marquee = boundsOf(
		screenToPlane(context, ...active.start),
		screenToPlane(context, clientX, active.last[1]),
	);
	// The direction is read on screen, which is the rectangle the operator drew, rather than in
	// plane coordinates, which some views mirror.
	const mode = marqueeMode(active.start[0], clientX);
	return [
		...new Set(
			context.entities
				.filter((entity) => entity.selectable)
				.filter((entity) =>
					marqueeCatches(
						entityBounds(entity, context.view, context.rotationQuarterTurns),
						marquee,
						mode,
					),
				)
				.map((entity) => entity.logicalFixtureId),
		),
	];
}

export function useCadViewportInteraction(
	context: CadViewportContext,
): CadViewportInteraction {
	const drag = useRef<Drag | null>(null);
	const [guide, setGuide] = useState<MoveAxis | null>(null);
	const [selectionBox, setSelectionBox] = useState<SelectionBox | null>(null);
	useEffect(() => {
		const shift = (spread: boolean) => (event: KeyboardEvent) => {
			if (event.key !== "Shift") return;
			const active = drag.current;
			if (active?.type !== "move" || active.axis === "plane") return;
			updateMovePreview(context, active, ...active.last, spread);
		};
		const keyDown = shift(true);
		const keyUp = shift(false);
		window.addEventListener("keydown", keyDown);
		window.addEventListener("keyup", keyUp);
		return () => {
			window.removeEventListener("keydown", keyDown);
			window.removeEventListener("keyup", keyUp);
		};
	});

	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
		context.canvas.current?.setPointerCapture(event.pointerId);
		drag.current = beginDrag(context, event, setGuide);
	}

	function pointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		if (!active) return;
		const dx = event.clientX - active.start[0];
		const dy = event.clientY - active.start[1];
		active.last = [event.clientX, event.clientY];
		if (active.type === "pan") {
			const start = active.startCamera ?? context.camera;
			context.onCamera({
				...start,
				pan: [start.pan[0] + dx / start.zoom, start.pan[1] - dy / start.zoom],
			});
			return;
		}
		if (active.type === "box") {
			// A press that never travels is a click on a fixture, not a marquee.
			if (Math.hypot(dx, dy) < 3 && !active.marquee) return;
			active.marquee = true;
			setSelectionBox({
				start: screenToPlane(context, ...active.start),
				end: screenToPlane(context, event.clientX, event.clientY),
			});
			return;
		}
		updateMovePreview(context, active, event.clientX, event.clientY, event.shiftKey);
	}

	async function pointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		drag.current = null;
		context.canvas.current?.releasePointerCapture(event.pointerId);
		if (active?.type === "box") {
			setSelectionBox(null);
			active.last = [event.clientX, event.clientY];
			context.onSelection({
				type: active.marquee
					? active.additive
						? "add"
						: "replace"
					: active.additive
						? "toggle"
						: "replace",
				ids: active.marquee
					? marqueeSelection(context, active, event.clientX)
					: active.hitId
						? [active.hitId]
						: [],
			});
			return;
		}
		if (active?.type !== "move") return;
		active.spread = active.axis !== "plane" && event.shiftKey;
		const current = active.deltaMillimetres ?? [0, 0, 0];
		context.onPreview(null);
		setGuide(null);
		// Under a millimetre is a click that slipped, not a move the operator meant.
		if (Math.hypot(...current) < 1) return;
		await context.onMove(
			current,
			active.entityIds ?? context.selectedIds,
			active.spread ?? false,
		);
	}

	function cancel() {
		drag.current = null;
		context.onPreview(null);
		setGuide(null);
		setSelectionBox(null);
	}

	return { guide, selectionBox, pointerDown, pointerMove, pointerUp, cancel };
}
