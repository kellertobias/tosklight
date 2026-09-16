/**
 * Drawing on a CAD viewport with the toolbar's tools: a line point by point, a box from one corner
 * click to the opposite corner's, a measurement by dragging, text where the operator clicks, and
 * erasing whatever the pointer is over.
 *
 * Panning stays where it always is — the middle button or Alt — so a line in progress can be moved
 * around without being dropped. Enter, a double click or a right-click finishes a line, a click on
 * its first point closes it, and Escape drops what is in progress or, with nothing in progress,
 * puts the tool down. With snapping on, the points snap (see `drawingSnap`); Shift, held, turns
 * every drawing snap off.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import type { CadAnnotation, CadAnnotationKind } from "./annotations";
import {
	annotationsForView,
	hitAnnotation,
	storedPoint,
	viewPoint,
} from "./annotationGeometry";
import { type CadDrawTool, useCadTools } from "./cadTools";
import { drawingCorners, snapDrawingPoint } from "./drawingSnap";
import type { PlanPoint } from "./projection";
import { snapPlanPoint, snapThreshold } from "./snapping";
import type { CadEntity, CadViewDirection, TileCamera } from "./types";

/** How near, in screen pixels, a click must be to close a line or to erase an item. */
const CATCH_PIXELS = 8;

/** How far, in screen pixels, a drag must travel to draw a box or a measurement. */
const DRAG_PIXELS = 3;

type Pointer = React.PointerEvent<HTMLCanvasElement>;
type StoredPoint = [number, number];

export interface CadDrawingTool {
	/** True while a tool other than Select is in hand. */
	active: boolean;
	/** The item in progress, drawn with the pointer as its last point. */
	draft: CadAnnotation | null;
	/** Where text is about to be placed on the tile, while its words are typed. */
	pendingText: PlanPoint | null;
	/** Where the point under the pointer has snapped onto a corner or connector, on the tile's plan. */
	snapMarker: PlanPoint | null;
	/** Handles a press; false leaves it to selection and panning. */
	pointerDown(event: Pointer): boolean;
	pointerMove(event: Pointer): void;
	/** Finishes a drag; false leaves the release to selection and panning. */
	pointerUp(event: Pointer): boolean;
	doubleClick(): void;
	/** A right-click finishes a line in progress; the browser's own menu never opens over a tool. */
	contextMenu(event: React.MouseEvent<HTMLCanvasElement>): void;
	commitText(text: string): void;
	cancelText(): void;
}

export interface DrawingViewport {
	canvas: React.RefObject<HTMLCanvasElement | null>;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	/** Print mode draws nothing. */
	enabled: boolean;
	/** What drawn points snap onto, when `snapping` is on; Shift turns it off while held. */
	entities?: readonly CadEntity[];
	snapping?: boolean;
}

function newAnnotation(
	view: CadViewDirection,
	kind: CadAnnotationKind,
	points: StoredPoint[],
	extra: Partial<CadAnnotation> = {},
): CadAnnotation {
	return {
		id: "",
		view,
		kind,
		points,
		closed: false,
		text: "",
		textHeightMillimetres: 250,
		...extra,
	};
}

/** A double click lands on the point the last click already put down; keep it once. */
function withoutRepeats(points: readonly StoredPoint[]): StoredPoint[] {
	return points.filter(
		(point, index) =>
			index === 0 ||
			point[0] !== points[index - 1][0] ||
			point[1] !== points[index - 1][1],
	);
}

/** Where the pointer is on the tile's plan, in millimetres. */
function planPointOf(viewport: DrawingViewport, event: Pointer): PlanPoint {
	const { camera } = viewport;
	const bounds = viewport.canvas.current?.getBoundingClientRect();
	if (!bounds) return [0, 0];
	return [
		(event.clientX - bounds.left - bounds.width / 2) / camera.zoom - camera.pan[0],
		-(event.clientY - bounds.top - bounds.height / 2) / camera.zoom - camera.pan[1],
	];
}

/**
 * Where a drawn point snaps — a measurement's onto the nearest snap point, a line's or a box's as
 * `snapDrawingPoint` says — unless snapping is off or Shift is held, and the marker that shows a
 * corner fit under the pointer. `anchor` is a line's last point, which its next one levels with.
 */
function usePointSnap(viewport: DrawingViewport, tool: CadDrawTool, anchor: StoredPoint | null) {
	const { view, rotationQuarterTurns } = viewport;
	const [marker, setMarker] = useState<PlanPoint | null>(null);
	const corners = useMemo(() => drawingCorners(viewport.entities ?? []), [viewport.entities]);
	const snap = (event: Pointer): { point: PlanPoint; marker: PlanPoint | null } => {
		const point = planPointOf(viewport, event);
		if (!viewport.snapping || event.shiftKey) return { point, marker: null };
		const threshold = snapThreshold(viewport.camera.zoom);
		if (tool === "measure") {
			const fit = snapPlanPoint(viewport.entities ?? [], point, view, rotationQuarterTurns, threshold);
			return { point: fit ?? point, marker: fit };
		}
		if (tool !== "polyline" && tool !== "box") return { point, marker: null };
		const from = tool === "polyline" && anchor ? viewPoint(view, anchor, rotationQuarterTurns) : null;
		return snapDrawingPoint(corners, point, view, rotationQuarterTurns, threshold, from);
	};
	const show = (point: PlanPoint | null) => {
		if (point?.[0] !== marker?.[0] || point?.[1] !== marker?.[1]) setMarker(point);
	};
	return { marker, snap, show };
}

/** Text starts at a size that reads well at the zoom it was placed at, in whole centimetres. */
function textAnnotation(
	view: CadViewDirection,
	anchor: StoredPoint,
	text: string,
	zoom: number,
): CadAnnotation {
	return newAnnotation(view, "text", [anchor], {
		text: text.trim(),
		textHeightMillimetres: Math.max(10, Math.round(16 / zoom / 10) * 10),
	});
}

/** The line, box or measurement in progress, drawn to the pointer; null before it has two points. */
function draftOf(
	view: CadViewDirection,
	tool: CadDrawTool,
	points: StoredPoint[],
): CadAnnotation | null {
	const drawn = tool === "polyline" || tool === "box" || tool === "measure";
	return drawn && points.length >= 2 ? { ...newAnnotation(view, tool, points), id: "draft" } : null;
}

/** Enter and Escape while a tool is in hand, except while a field is being typed in. */
function useDrawingKeys(active: boolean, onKey: (key: string) => void) {
	useEffect(() => {
		if (!active) return;
		const key = (event: KeyboardEvent) => {
			if ((event.target as Element | null)?.closest?.("input, textarea")) return;
			onKey(event.key);
		};
		window.addEventListener("keydown", key);
		return () => window.removeEventListener("keydown", key);
	});
}

export function useCadDrawingTool(viewport: DrawingViewport): CadDrawingTool {
	const tools = useCadTools();
	const { view, rotationQuarterTurns, camera } = viewport;
	const active = viewport.enabled && tools.tool !== "select";
	const [points, setPoints] = useState<StoredPoint[]>([]);
	const [cursor, setCursor] = useState<StoredPoint | null>(null);
	const [pendingText, setPendingText] = useState<StoredPoint | null>(null);
	const pointSnap = usePointSnap(viewport, tools.tool, points.at(-1) ?? null);
	const dragging = useRef(false);

	function reset() {
		setPoints([]);
		setCursor(null);
		setPendingText(null);
		pointSnap.show(null);
		dragging.current = false;
	}

	// Another tool, view or turn starts over rather than finishing a shape drawn for the last one.
	useEffect(reset, [tools.tool, view, rotationQuarterTurns]);

	const planPoint = (event: Pointer) => planPointOf(viewport, event);
	const stored = (event: Pointer) =>
		storedPoint(view, pointSnap.snap(event).point, rotationQuarterTurns);
	const pixelsApart = (a: StoredPoint, b: StoredPoint) =>
		Math.hypot(a[0] - b[0], a[1] - b[1]) * camera.zoom;

	function finishLine(closed: boolean) {
		const line = withoutRepeats(points);
		reset();
		if (line.length >= 2)
			void tools.save(
				newAnnotation(view, "polyline", line, { closed: closed && line.length >= 3 }),
			);
	}

	useDrawingKeys(active, (key) => {
		if (key === "Enter" && tools.tool === "polyline") finishLine(false);
		if (key !== "Escape") return;
		if (points.length || pendingText) reset();
		else tools.setTool("select");
	});

	function finishBox(corner: StoredPoint) {
		const start = points[0];
		// A second click on the first corner is not a box yet; the first corner stays.
		if (pixelsApart(start, corner) < DRAG_PIXELS) return;
		reset();
		void tools.save(newAnnotation(view, "box", [start, corner]));
	}

	function pointerDown(event: Pointer) {
		if (!active) return false;
		// A right-click (or a Control-click) belongs to the context menu, which finishes a line.
		if (event.button === 2 || (event.button === 0 && event.ctrlKey)) return true;
		if (event.button !== 0 || event.altKey) return false;
		const point = stored(event);
		switch (tools.tool) {
			case "polyline":
				if (points.length >= 3 && pixelsApart(points[0], point) <= CATCH_PIXELS)
					finishLine(true);
				else setPoints([...points, point]);
				return true;
			case "box":
				if (points.length) finishBox(point);
				else {
					setPoints([point]);
					setCursor(point);
				}
				return true;
			case "measure":
				viewport.canvas.current?.setPointerCapture(event.pointerId);
				dragging.current = true;
				setPoints([point]);
				setCursor(point);
				return true;
			case "text":
				setPendingText(point);
				return true;
			case "erase": {
				const id = hitAnnotation(
					annotationsForView(tools.annotations, view),
					planPoint(event),
					rotationQuarterTurns,
					CATCH_PIXELS / camera.zoom,
				);
				if (id) void tools.remove(id);
				return true;
			}
			default:
				return false;
		}
	}

	function pointerMove(event: Pointer) {
		if (!active) return;
		const drawing = tools.tool === "polyline" || tools.tool === "box";
		if (drawing || tools.tool === "measure") pointSnap.show(pointSnap.snap(event).marker);
		if (dragging.current || (drawing && points.length))
			setCursor(stored(event));
	}

	function pointerUp(event: Pointer) {
		if (!dragging.current) return false;
		const start = points[0];
		const end = stored(event);
		viewport.canvas.current?.releasePointerCapture(event.pointerId);
		reset();
		if (start && pixelsApart(start, end) >= DRAG_PIXELS)
			void tools.save(newAnnotation(view, "measure", [start, end]));
		return true;
	}

	function commitText(text: string) {
		const anchor = pendingText;
		setPendingText(null);
		if (anchor && text.trim()) void tools.save(textAnnotation(view, anchor, text, camera.zoom));
	}

	return {
		active,
		draft: active ? draftOf(view, tools.tool, cursor ? [...points, cursor] : points) : null,
		pendingText: pendingText
			? viewPoint(view, pendingText, rotationQuarterTurns)
			: null,
		snapMarker: active ? pointSnap.marker : null,
		pointerDown,
		pointerMove,
		pointerUp,
		doubleClick: () => {
			if (active && tools.tool === "polyline") finishLine(false);
		},
		contextMenu: (event) => {
			if (!active) return;
			event.preventDefault();
			if (tools.tool === "polyline") finishLine(false);
			else if (tools.tool === "box") reset();
		},
		commitText,
		cancelText: () => setPendingText(null),
	};
}
