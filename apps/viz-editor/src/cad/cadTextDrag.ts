/**
 * Picking and moving placed text with the Select tool.
 *
 * A press on text picks it, and picks it alone: the element drawn under the words is left
 * unselected, and the rig's selection is cleared so Info turns to the text. Dragging the text, or
 * the gizmo that stands on its anchor once it is picked, moves it — freely from the words or the
 * gizmo's square, along one axis from an arrow — and letting go writes the move as one step Undo
 * puts back. A press anywhere else puts the text down and is left to the rig.
 */
import { useRef } from "react";
import type { CadAnnotation } from "./annotations";
import { hitAnnotation, storedPoint, viewPoint } from "./annotationGeometry";
import { useCadTools } from "./cadTools";
import type { PlanPoint } from "./projection";
import type { CadViewDirection, SelectionChange, TileCamera } from "./types";

type Axis = "plane" | "horizontal" | "vertical";

interface TextDrag {
	id: string;
	annotation: CadAnnotation;
	start: [number, number];
	axis: Axis;
	points: [number, number][];
}

/** The gizmo arrows' length and the square's reach, in screen pixels, as the rig's gizmo has. */
const GIZMO_LENGTH = 48;
const GIZMO_REACH = 10;

/** Which part of a gizmo standing at `origin` a press takes, or null. */
export function textGizmoAxis(
	point: PlanPoint,
	origin: PlanPoint,
	zoom: number,
): Axis | null {
	const reach = GIZMO_REACH / zoom;
	const length = GIZMO_LENGTH / zoom;
	const [dx, dy] = [point[0] - origin[0], point[1] - origin[1]];
	if (Math.hypot(dx, dy) <= reach) return "plane";
	if (dx >= 0 && dx <= length && Math.abs(dy) <= reach) return "horizontal";
	if (dy >= 0 && dy <= length && Math.abs(dx) <= reach) return "vertical";
	return null;
}

/** A text's points moved by a delta on the tile, stored as every drawn point is. */
export function movedText(
	annotation: CadAnnotation,
	delta: PlanPoint,
	rotationQuarterTurns: number,
): [number, number][] {
	return annotation.points.map((point) => {
		const [x, y] = viewPoint(annotation.view, point, rotationQuarterTurns);
		const moved = storedPoint(annotation.view, [x + delta[0], y + delta[1]], rotationQuarterTurns);
		return [Math.round(moved[0]), Math.round(moved[1])];
	});
}

export function useTextDrag({
	canvas,
	view,
	rotationQuarterTurns,
	camera,
	enabled,
	onSelection,
}: {
	canvas: React.RefObject<HTMLCanvasElement | null>;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	enabled: boolean;
	onSelection(change: SelectionChange): void;
}) {
	const tools = useCadTools();
	const active = useRef<TextDrag | null>(null);
	// Text is where it is drawn: a move the show has not taken yet already carries its anchor, so the
	// gizmo stays on the words and the next press picks them up from there, never from where they were.
	const preview = tools.textPreview;
	const texts = tools.annotations
		.filter((each) => each.view === view && each.kind === "text")
		.map((each) => (preview?.id === each.id ? { ...each, points: preview.points } : each));
	const picked = texts.find((each) => each.id === tools.selectedTextId) ?? null;

	const toPlan = (clientX: number, clientY: number): PlanPoint => {
		const bounds = canvas.current?.getBoundingClientRect();
		if (!bounds) return [0, 0];
		return [
			(clientX - bounds.left - bounds.width / 2) / camera.zoom - camera.pan[0],
			-(clientY - bounds.top - bounds.height / 2) / camera.zoom - camera.pan[1],
		];
	};

	/** Where the picked text's gizmo stands on this tile: its anchor, wherever the words are drawn. */
	const gizmo: PlanPoint | null = picked?.points[0]
		? viewPoint(view, picked.points[0], rotationQuarterTurns)
		: null;

	/** Takes a press on text or on the picked text's gizmo; false leaves the press to the rig. */
	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>): boolean {
		if (!enabled || tools.tool !== "select" || event.button !== 0 || event.altKey) return false;
		const point = toPlan(event.clientX, event.clientY);
		const onGizmo = gizmo ? textGizmoAxis(point, gizmo, camera.zoom) : null;
		const hit = onGizmo
			? (picked?.id ?? null)
			: hitAnnotation(texts, point, rotationQuarterTurns, 6 / camera.zoom);
		const annotation = texts.find((each) => each.id === hit);
		if (!annotation) {
			if (tools.selectedTextId) tools.selectText(null);
			return false;
		}
		canvas.current?.setPointerCapture(event.pointerId);
		if (tools.selectedTextId !== annotation.id) {
			tools.selectText(annotation.id);
			onSelection({ type: "replace", ids: [] });
		}
		active.current = {
			id: annotation.id,
			annotation,
			start: [event.clientX, event.clientY],
			axis: onGizmo ?? "plane",
			points: annotation.points,
		};
		return true;
	}

	/** Moves the text being dragged with the pointer; false when no text is being dragged. */
	function pointerMove(event: React.PointerEvent<HTMLCanvasElement>): boolean {
		const current = active.current;
		if (!current) return false;
		const dx = (event.clientX - current.start[0]) / camera.zoom;
		const dy = -(event.clientY - current.start[1]) / camera.zoom;
		const delta: PlanPoint = [
			current.axis === "vertical" ? 0 : dx,
			current.axis === "horizontal" ? 0 : dy,
		];
		current.points = movedText(current.annotation, delta, rotationQuarterTurns);
		tools.setTextPreview({ id: current.id, points: current.points });
		return true;
	}

	/** Writes the move a release leaves; false when no text was being dragged. */
	function pointerUp(event: React.PointerEvent<HTMLCanvasElement>): boolean {
		const current = active.current;
		if (!current) return false;
		pointerMove(event);
		active.current = null;
		canvas.current?.releasePointerCapture(event.pointerId);
		const moved = current.points.some(
			(point, index) =>
				point[0] !== current.annotation.points[index][0] ||
				point[1] !== current.annotation.points[index][1],
		);
		if (moved)
			void tools
				.change({ ...current.annotation, points: current.points })
				.finally(() => tools.setTextPreview(null));
		else tools.setTextPreview(null);
		return true;
	}

	return { gizmo, selectedId: picked?.id ?? null, pointerDown, pointerMove, pointerUp };
}
