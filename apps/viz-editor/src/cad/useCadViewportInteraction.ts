/**
 * The pointer on a CAD viewport: panning, picking, the move gizmo and the marquee.
 *
 * The drag in flight lives in a ref rather than in state, because every pointer move would
 * otherwise re-render the whole viewport while the operator is still dragging. What the renderer
 * needs to draw — the axis guide and the marquee rectangle — is state, and nothing else is.
 */
import { useEffect, useRef, useState } from "react";
import { duplicateStep, rememberMoveAxis } from "./duplicateStep";
import type { CadObjectMenuRequest } from "./CadObjectMenu";
import type { SelectionBox, SnapGuide } from "./lineRenderer";
import {
	boundsOf,
	entityBounds,
	marqueeCatches,
	marqueeMode,
} from "./marqueeSelection";
import { type EntryAxis, type MoveReadout, moveOrigin } from "./moveEntry";
import { typedDelta, useCadMoveEntry } from "./useCadMoveEntry";
import { type RotateDrag, useRotateHandle } from "./cadRotateDrag";
import { holdsDuplicateModifier, isDuplicateModifierKey } from "./cadModifiers";
import { type MoveAxis, pickEntity, pickGizmo } from "./planGeometry";
import type { PlanPoint } from "./projection";
import { type FreeAxes, snapMove, snapThreshold } from "./snapping";
import type {
	CadDrawing,
	CadEntity,
	CadTransformPreview,
	CadViewDirection,
	SelectionChange,
	TileCamera,
} from "./types";
import { planeDelta, projectPoint } from "./types";

const NO_SNAP: { markers: PlanPoint[]; guides: SnapGuide[] } = { markers: [], guides: [] };

export interface Drag {
	type: "pan" | "move" | "box" | "rotate";
	/** A turn of the gizmo's rotate handle in flight. */
	rotate?: RotateDrag;
	start: [number, number];
	last: [number, number];
	axis: MoveAxis;
	entityIds?: readonly string[];
	startCamera?: TileCamera;
	additive?: boolean;
	deltaMillimetres?: [number, number, number];
	/** The drag as the pointer moved it, before snapping: under a millimetre is still a click. */
	rawDeltaMillimetres?: [number, number, number];
	spread?: boolean;
	hitId?: string;
	/** The placement under the pointer: the fixture itself, or one of its multi-patch copies. */
	hitEntityId?: string;
	marquee?: boolean;
	/** The world point the gizmo stood on when the move began, in millimetres. */
	origin?: [number, number, number];
	/** A coordinate typed while dragging; empty until the operator types. */
	entry?: string;
	/** On a free drag, the screen axis a typed value applies to; Tab switches it. */
	entryAxis?: EntryAxis;
	/**
	 * Whether the move places a copy instead: whether the duplicate modifier is held now. It follows
	 * the key as it goes down and up, and the release commits a copy only while it is still held.
	 */
	duplicate?: boolean;
}

export interface CadViewportInteraction {
	guide: MoveAxis | null;
	selectionBox: SelectionBox | null;
	/** Where the move in flight has snapped onto a fit, on this tile's plan. */
	snapMarkers: readonly PlanPoint[];
	/** The sides the move in flight has lined up, as lines on this tile's plan. */
	snapGuides: readonly SnapGuide[];
	/** The live coordinates and typed entry beside the gizmo while a move is in flight. */
	readout: MoveReadout | null;
	pointerDown(event: React.PointerEvent<HTMLCanvasElement>): void;
	pointerMove(event: React.PointerEvent<HTMLCanvasElement>): void;
	pointerUp(event: React.PointerEvent<HTMLCanvasElement>): Promise<void>;
	/** Abandon a drag the pointer left behind, leaving the rig where it started. */
	cancel(): void;
	/** A right-click: opens the object menu over a selectable element, selecting it first. */
	contextMenu(event: React.MouseEvent<HTMLCanvasElement>): void;
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
	/** Whether a move snaps onto a fit (Settings → Enable snapping); Shift turns it off while held. */
	snapping?: boolean;
	/** The camera while a pan is in flight; `onCameraEnd` hands the last one on when it finishes. */
	onCamera(camera: TileCamera): void;
	onCameraEnd?(): void;
	onSelection(change: SelectionChange): void;
	/** Widens a plain pick to whole Venue element groups; Shift picks elements alone. */
	expandSelection?(ids: readonly string[]): string[];
	/**
	 * Which placement a click picked. The selection names whole fixtures, so this is how a panel
	 * that edits one copy of a multi-patched fixture knows which copy was meant.
	 */
	onFocusEntity?(entityId: string | null): void;
	onPreview(preview: CadTransformPreview | null): void;
	/** Opens the Duplicate / Delete menu for the selection. */
	onObjectMenu?(request: CadObjectMenuRequest): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
		/** False while Shift is held: nothing snaps and a lamp is not mounted onto a truss. */
		snap: boolean,
	): Promise<void>;
	/** Commits a turn of the gizmo's rotate handle; absent, the handle takes no press. */
	onTransforms?(placements: NonNullable<CadTransformPreview["placements"]>): Promise<void>;
	/** Places copies of `entityIds` moved by `deltaMillimetres`; absent, a move never duplicates. */
	onDuplicateMove?(deltaMillimetres: [number, number, number], entityIds: readonly string[]): Promise<void>;
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

/** The plan axes a drag along `axis` of this tile can change. */
export function freeAxes(
	axis: MoveAxis,
	view: CadViewDirection,
	rotationQuarterTurns: number,
): FreeAxes {
	const free = [false, false, false];
	const reach = (local: [number, number]) =>
		planeDelta(local, view, rotationQuarterTurns).forEach((value, index) => {
			if (value !== 0) free[index] = true;
		});
	if (axis !== "vertical") reach([1, 0]);
	if (axis !== "horizontal") reach([0, 1]);
	return free as unknown as FreeAxes;
}

/**
 * What a drag of the gizmo has moved the selection by, and the preview that shows it. Shift spreads
 * an arrow drag and, on any drag, turns snapping off.
 */
function updateMovePreview(
	context: CadViewportContext,
	active: Drag,
	clientX: number,
	clientY: number,
	shift: boolean,
	showSnap: (markers: PlanPoint[], guides?: SnapGuide[]) => void,
) {
	const { camera, view, rotationQuarterTurns, selectedIds, onPreview } = context;
	const dx = clientX - active.start[0];
	const dy = clientY - active.start[1];
	const localDelta: [number, number] = [
		active.axis === "vertical" ? 0 : dx / camera.zoom,
		active.axis === "horizontal" ? 0 : -dy / camera.zoom,
	];
	const raw = planeDelta(localDelta, view, rotationQuarterTurns);
	const entityIds = active.entityIds ?? selectedIds;
	active.rawDeltaMillimetres = raw;
	active.spread = active.axis !== "plane" && shift && !active.duplicate;
	// A spread move fans the selection out, so there is no one fit for it to snap onto.
	const snapped =
		context.snapping && !shift && !active.spread && Math.hypot(...raw) >= 1
			? snapMove(
					context.entities,
					entityIds,
					raw,
					freeAxes(active.axis, view, rotationQuarterTurns),
					snapThreshold(camera.zoom),
				)
			: { delta: raw, targets: [], guides: [] };
	active.deltaMillimetres = snapped.delta;
	const onPlan = (point: [number, number, number]) => projectPoint(point, view, rotationQuarterTurns);
	showSnap(
		snapped.targets.map(onPlan),
		snapped.guides.map(([start, end]): SnapGuide => [onPlan(start), onPlan(end)]),
	);
	onPreview({
		entityIds,
		deltaMillimetres: snapped.delta,
		spread: active.spread,
		...(active.duplicate && context.onDuplicateMove ? { duplicate: true } : {}),
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
	// A press on the gizmo moves, even with Option held, which is the Mac's duplicate modifier; a
	// press anywhere else with Option, or the middle button, pans.
	const axis =
		event.button === 0 && context.editEnabled
			? pickGizmo(
					screenToPlane(context, event.clientX, event.clientY),
					entities,
					selected,
					view,
					rotationQuarterTurns,
					camera,
				)
			: null;
	if (!axis && (event.button === 1 || event.altKey))
		return {
			type: "pan",
			start,
			last: start,
			axis: "plane",
			startCamera: camera,
		};
	if (!context.editEnabled) return null;
	if (axis) {
		const selectable = new Set(
			entities
				.filter((entity) => entity.selectable)
				.map((entity) => entity.logicalFixtureId),
		);
		setGuide(axis);
		const entityIds = selectedIds.filter((id) => selectable.has(id));
		return {
			type: "move",
			start,
			last: start,
			axis,
			entityIds,
			origin: moveOrigin(entities, entityIds) ?? undefined,
			entry: "",
			entryAxis: "horizontal",
			spread: axis !== "plane" && event.shiftKey,
			duplicate: holdsDuplicateModifier(event),
			// The gizmo stands on the selection's origin, so a press there that never moves is
			// still a click on the element beneath it.
			additive: event.shiftKey,
			hitId: hit?.logicalFixtureId,
			hitEntityId: hit?.id,
		};
	}
	return {
		type: "box",
		start,
		last: start,
		axis: "plane",
		additive: event.shiftKey,
		hitId: hit?.logicalFixtureId,
		hitEntityId: hit?.id,
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

/**
 * A right-click over a selectable element opens the object menu. Inside the selection it keeps the
 * selection, so a whole selection can be duplicated or deleted; outside it, the element under the
 * pointer (with its group, unless Shift is held) becomes the selection first.
 */
function openObjectMenu(context: CadViewportContext, event: React.MouseEvent<HTMLCanvasElement>) {
	event.preventDefault();
	// A Control-click is a click here, not a right-click; only the right button opens the menu.
	if (event.button !== 2 || event.ctrlKey || !context.editEnabled || !context.onObjectMenu) return;
	const hit = pickEntity(
		screenToPlane(context, event.clientX, event.clientY),
		context.entities,
		context.drawingById,
		context.view,
		context.rotationQuarterTurns,
		context.camera,
	);
	if (!hit) return;
	let entityIds: readonly string[] = context.selectedIds;
	if (!context.selected.has(hit.logicalFixtureId)) {
		const ids = [hit.logicalFixtureId];
		entityIds =
			event.shiftKey || !context.expandSelection ? ids : context.expandSelection(ids);
		context.onFocusEntity?.(hit.id);
		context.onSelection({ type: "replace", ids: [...entityIds] });
	}
	context.onObjectMenu({
		x: event.clientX,
		y: event.clientY,
		duplicateOffset: duplicateStep(
			context.entities,
			entityIds,
			context.view,
			context.rotationQuarterTurns,
		),
		entityIds,
	});
}

/** A plain pick takes whole groups; with Shift (`additive`) each element is taken alone. */
function picked(
	context: CadViewportContext,
	ids: string[],
	additive: boolean | undefined,
): string[] {
	return additive || !context.expandSelection ? ids : context.expandSelection(ids);
}

/** A released selection box: a marquee selects what it caught, a click what it hit. */
function finishBox(
	context: CadViewportContext,
	active: Drag,
	clientX: number,
	clientY: number,
) {
	active.last = [clientX, clientY];
	if (!active.marquee) context.onFocusEntity?.(active.hitEntityId ?? null);
	context.onSelection({
		type: active.marquee
			? active.additive
				? "add"
				: "replace"
			: active.additive
				? "toggle"
				: "replace",
		ids: picked(
			context,
			active.marquee
				? marqueeSelection(context, active, clientX)
				: active.hitId
					? [active.hitId]
					: [],
			active.additive,
		),
	});
}

/**
 * Calls `onChange` whenever Shift is pressed or let go, with whether it is now held, and
 * `onDuplicate` whenever the platform's duplicate modifier is, with whether it is and with Shift.
 */
function useShiftChanges(
	onChange: (held: boolean) => void,
	onDuplicate: (held: boolean, shift: boolean) => void,
) {
	useEffect(() => {
		const shift = (held: boolean) => (event: KeyboardEvent) => {
			if (event.key === "Shift") onChange(held);
			if (isDuplicateModifierKey(event.key)) onDuplicate(held, event.shiftKey);
		};
		const [down, up] = [shift(true), shift(false)];
		window.addEventListener("keydown", down);
		window.addEventListener("keyup", up);
		return () => {
			window.removeEventListener("keydown", down);
			window.removeEventListener("keyup", up);
		};
	});
}

/** The preview turns into a copy, or back into the move, the moment the modifier changes. */
function followDuplicateKey(
	context: CadViewportContext,
	active: Drag | null,
	held: boolean,
	shift: boolean,
	showSnap: (markers: PlanPoint[], guides?: SnapGuide[]) => void,
) {
	if (active?.type !== "move" || active.duplicate === held) return;
	active.duplicate = held;
	if (active.rawDeltaMillimetres) updateMovePreview(context, active, ...active.last, shift, showSnap);
}

export function useCadViewportInteraction(
	context: CadViewportContext,
): CadViewportInteraction {
	const drag = useRef<Drag | null>(null);
	const [guide, setGuide] = useState<MoveAxis | null>(null);
	const [selectionBox, setSelectionBox] = useState<SelectionBox | null>(null);
	const [snap, setSnap] = useState<{ markers: PlanPoint[]; guides: SnapGuide[] }>(NO_SNAP);
	const turning = useRotateHandle(context, (point) => screenToPlane(context, ...point));
	const shownSnap = useRef<string>(JSON.stringify([[], []]));
	const { readout, refresh, commitTyped, clearReadout } = useCadMoveEntry({
		drag,
		context,
		cancel: () => cancel(),
		endMove: () => {
			setGuide(null);
			showSnap([]);
		},
	});
	// Set only when the markers change, so a drag that stays snapped does not re-render every move.
	function showSnap(markers: PlanPoint[], guides: SnapGuide[] = []) {
		const key = JSON.stringify([markers, guides]);
		if (key === shownSnap.current) return;
		shownSnap.current = key;
		setSnap({ markers, guides });
	}
	// Shift pressed or let go mid-drag changes the drag at once: a turn goes free, a move spreads.
	useShiftChanges((held) => {
		const active = drag.current;
		if (active?.type === "rotate") return turning.turnTo(active, active.last, held);
		if (active?.type !== "move" || !active.rawDeltaMillimetres) return;
		updateMovePreview(context, active, ...active.last, held, showSnap);
		refresh(active);
	}, (held, shift) => followDuplicateKey(context, drag.current, held, shift, showSnap));

	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
		// The right button picks for the object menu, which `contextMenu` handles; it drags nothing.
		if (event.button === 2) return;
		context.canvas.current?.setPointerCapture(event.pointerId);
		drag.current = turning.begin(event) ?? beginDrag(context, event, setGuide);
	}

	function pointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		if (!active) return;
		const dx = event.clientX - active.start[0];
		const dy = event.clientY - active.start[1];
		active.last = [event.clientX, event.clientY];
		if (active.type === "rotate") return turning.turnTo(active, active.last, event.shiftKey);
		if (active.type === "move") active.duplicate = holdsDuplicateModifier(event);
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
		updateMovePreview(context, active, event.clientX, event.clientY, event.shiftKey, showSnap);
		refresh(active);
	}

	async function pointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
		const active = drag.current;
		drag.current = null;
		context.canvas.current?.releasePointerCapture(event.pointerId);
		if (active?.type === "pan") {
			context.onCameraEnd?.();
			return;
		}
		if (active?.type === "box") {
			setSelectionBox(null);
			finishBox(context, active, event.clientX, event.clientY);
			return;
		}
		if (active?.type === "rotate") return turning.finish(active, event);
		if (active?.type !== "move") return;
		if (active.entry) {
			// Letting go mid-entry keeps a valid typed coordinate; half-typed text moves nothing.
			if (typedDelta(context, active)) await commitTyped(active);
			else cancel();
			return;
		}
		clearReadout();
		// Only a modifier still held as the mouse lets go places a copy; let go first, it is a move.
		active.duplicate = holdsDuplicateModifier(event);
		// Shift may have been pressed or let go since the last move; the release decides.
		if (active.rawDeltaMillimetres)
			updateMovePreview(context, active, event.clientX, event.clientY, event.shiftKey, showSnap);
		active.spread = active.axis !== "plane" && event.shiftKey;
		const current = active.deltaMillimetres ?? [0, 0, 0];
		setGuide(null);
		showSnap([]);
		// Under a millimetre is a click that slipped, not a move the operator meant.
		if (Math.hypot(...(active.rawDeltaMillimetres ?? [0, 0, 0])) < 1) {
			context.onPreview(null);
			if (active.axis === "plane" && active.hitId) {
				context.onFocusEntity?.(active.hitEntityId ?? null);
				context.onSelection({
					type: active.additive ? "toggle" : "replace",
					ids: picked(context, [active.hitId], active.additive),
				});
			}
			return;
		}
		// The preview stays where the operator let go: the move clears it once the show has answered,
		// in the same render that draws the committed positions.
		const moved = active.entityIds ?? context.selectedIds;
		rememberMoveAxis(moved, current, context.view, context.rotationQuarterTurns);
		if (active.duplicate && context.onDuplicateMove) return context.onDuplicateMove(current, moved);
		await context.onMove(current, moved, active.spread ?? false, !event.shiftKey);
	}

	function cancel() {
		if (drag.current?.type === "pan") context.onCameraEnd?.();
		drag.current = null;
		context.onPreview(null);
		setGuide(null);
		setSelectionBox(null);
		clearReadout();
		turning.clear();
		showSnap([]);
	}

	const contextMenu = (event: React.MouseEvent<HTMLCanvasElement>) => openObjectMenu(context, event);
	return {
		guide,
		selectionBox,
		snapMarkers: snap.markers,
		snapGuides: snap.guides,
		readout: turning.readout ?? readout,
		pointerDown,
		pointerMove,
		pointerUp,
		cancel,
		contextMenu,
	};
}
