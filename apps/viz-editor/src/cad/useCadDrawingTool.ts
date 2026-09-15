/**
 * Drawing on a CAD viewport with the toolbar's tools: a line point by point, a box or a measurement
 * by dragging, text where the operator clicks, and erasing whatever the pointer is over.
 *
 * Panning stays where it always is — the middle button or Alt — so a line in progress can be moved
 * around without being dropped. Enter or a double click finishes a line, a click on its first
 * point closes it, and Escape drops what is in progress or, with nothing in progress, puts the
 * tool down.
 */
import { useEffect, useRef, useState } from "react";
import type { CadAnnotation, CadAnnotationKind } from "./annotations";
import {
	annotationsForView,
	hitAnnotation,
	storedPoint,
	viewPoint,
} from "./annotationGeometry";
import { useCadTools } from "./cadTools";
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
	/** Where the measurement's point under the pointer has snapped, on the tile's plan. */
	snapMarker: PlanPoint | null;
	/** Handles a press; false leaves it to selection and panning. */
	pointerDown(event: Pointer): boolean;
	pointerMove(event: Pointer): void;
	/** Finishes a drag; false leaves the release to selection and panning. */
	pointerUp(event: Pointer): boolean;
	doubleClick(): void;
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
	/** What a measurement's ends snap onto, when `snapping` is on; Shift turns it off while held. */
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
 * Where a measurement's point snaps — onto the nearest snap point in reach, unless snapping is off
 * or Shift is held — and the marker that shows the fit under the pointer.
 */
function useMeasureSnap(viewport: DrawingViewport, measuring: boolean) {
	const [marker, setMarker] = useState<PlanPoint | null>(null);
	const snap = (event: Pointer): PlanPoint | null =>
		measuring && viewport.snapping && !event.shiftKey
			? snapPlanPoint(
					viewport.entities ?? [],
					planPointOf(viewport, event),
					viewport.view,
					viewport.rotationQuarterTurns,
					snapThreshold(viewport.camera.zoom),
				)
			: null;
	const show = (point: PlanPoint | null) => {
		if (point?.[0] !== marker?.[0] || point?.[1] !== marker?.[1]) setMarker(point);
	};
	return { marker, snap, show };
}

export function useCadDrawingTool(viewport: DrawingViewport): CadDrawingTool {
	const tools = useCadTools();
	const { view, rotationQuarterTurns, camera } = viewport;
	const active = viewport.enabled && tools.tool !== "select";
	const [points, setPoints] = useState<StoredPoint[]>([]);
	const [cursor, setCursor] = useState<StoredPoint | null>(null);
	const [pendingText, setPendingText] = useState<StoredPoint | null>(null);
	const measure = useMeasureSnap(viewport, tools.tool === "measure");
	const dragging = useRef(false);

	function reset() {
		setPoints([]);
		setCursor(null);
		setPendingText(null);
		measure.show(null);
		dragging.current = false;
	}

	// Another tool, view or turn starts over rather than finishing a shape drawn for the last one.
	useEffect(reset, [tools.tool, view, rotationQuarterTurns]);

	const planPoint = (event: Pointer) => planPointOf(viewport, event);
	const stored = (event: Pointer) =>
		storedPoint(view, measure.snap(event) ?? planPoint(event), rotationQuarterTurns);
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

	useEffect(() => {
		if (!active) return;
		const key = (event: KeyboardEvent) => {
			if ((event.target as Element | null)?.closest?.("input, textarea")) return;
			if (event.key === "Enter" && tools.tool === "polyline") finishLine(false);
			if (event.key !== "Escape") return;
			if (points.length || pendingText) reset();
			else tools.setTool("select");
		};
		window.addEventListener("keydown", key);
		return () => window.removeEventListener("keydown", key);
	});

	function pointerDown(event: Pointer) {
		if (!active || event.button !== 0 || event.altKey) return false;
		const point = stored(event);
		switch (tools.tool) {
			case "polyline":
				if (points.length >= 3 && pixelsApart(points[0], point) <= CATCH_PIXELS)
					finishLine(true);
				else setPoints([...points, point]);
				return true;
			case "box":
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
		if (tools.tool === "measure") measure.show(measure.snap(event));
		if (dragging.current || (tools.tool === "polyline" && points.length))
			setCursor(stored(event));
	}

	function pointerUp(event: Pointer) {
		if (!dragging.current) return false;
		const start = points[0];
		const end = stored(event);
		const kind = tools.tool === "box" ? "box" : "measure";
		viewport.canvas.current?.releasePointerCapture(event.pointerId);
		reset();
		if (start && pixelsApart(start, end) >= DRAG_PIXELS)
			void tools.save(newAnnotation(view, kind, [start, end]));
		return true;
	}

	function commitText(text: string) {
		const anchor = pendingText;
		setPendingText(null);
		if (!anchor || !text.trim()) return;
		// Text starts at a size that reads well at the zoom it was placed at, in whole centimetres.
		const height = Math.max(10, Math.round(16 / camera.zoom / 10) * 10);
		void tools.save(
			newAnnotation(view, "text", [anchor], {
				text: text.trim(),
				textHeightMillimetres: height,
			}),
		);
	}

	const draftKind: CadAnnotationKind | null =
		tools.tool === "polyline" || tools.tool === "box" || tools.tool === "measure"
			? tools.tool
			: null;
	const draftPoints = cursor ? [...points, cursor] : points;
	return {
		active,
		draft:
			active && draftKind && draftPoints.length >= 2
				? { ...newAnnotation(view, draftKind, draftPoints), id: "draft" }
				: null,
		pendingText: pendingText
			? viewPoint(view, pendingText, rotationQuarterTurns)
			: null,
		snapMarker: active && tools.tool === "measure" ? measure.marker : null,
		pointerDown,
		pointerMove,
		pointerUp,
		doubleClick: () => {
			if (active && tools.tool === "polyline") finishLine(false);
		},
		commitText,
		cancelText: () => setPendingText(null),
	};
}
