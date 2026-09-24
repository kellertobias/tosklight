/**
 * A drag of the move gizmo's rotate handle: from the press on the arc to the turn it commits.
 *
 * The turn is measured as the page angle the pointer sweeps about the gizmo's pivot, carried onto
 * the view's own rotation component, and snapped to 15° steps unless Shift is held. Each move is
 * worked out from where the selection stood when the drag began, so the preview never feeds back
 * into the turn it shows. Letting go commits the turn as one step Undo puts back; a turn of
 * nothing commits nothing.
 */
import { useState } from "react";
import type { MoveReadout } from "./moveEntry";
import { gizmoGeometry } from "./planGeometry";
import {
	hitsRotateArc,
	pageAngle,
	pageTurnSign,
	ROTATION_AXIS_LABELS,
	rotationAxisIndex,
	snappedTurn,
	type TurnedPlacement,
	turnedPlacements,
} from "./gizmoRotation";
import type { CadEntity } from "./types";
import type { CadViewportContext, Drag } from "./useCadViewportInteraction";

type Vec3 = [number, number, number];

export interface RotateDrag {
	/** The fixtures turning, and where they stood when the drag began. */
	fixtureIds: readonly string[];
	entities: readonly CadEntity[];
	/** The point the selection turns about, in the world and on this page. */
	pivot: Vec3;
	pivotPage: [number, number];
	startAngle: number;
	/** The turn the drag shows now, in degrees on the view's rotation component. */
	turn: number;
}

/** The pivot a selection turns about: the middle of where its fixtures stand. */
function pivotOf(entities: readonly CadEntity[]): Vec3 {
	const middle = (axis: number) => {
		const values = entities.map((entity) => entity.positionMillimetres[axis]);
		return (Math.min(...values) + Math.max(...values)) / 2;
	};
	return [middle(0), middle(1), middle(2)];
}

/** A rotate drag when the press lands on the gizmo's arc; null leaves the press to the others. */
export function beginRotate(
	context: CadViewportContext,
	point: [number, number],
): RotateDrag | null {
	const { entities, selected, view, rotationQuarterTurns, camera } = context;
	if (!context.editEnabled || !context.onTransforms) return null;
	const gizmo = gizmoGeometry(entities, selected, view, rotationQuarterTurns, camera);
	if (!gizmo || !hitsRotateArc(point, gizmo.origin, gizmo.length, camera.zoom)) return null;
	const turning = entities.filter(
		(entity) =>
			entity.selectable &&
			selected.has(entity.logicalFixtureId) &&
			entity.id === entity.logicalFixtureId,
	);
	if (!turning.length) return null;
	return {
		fixtureIds: [...new Set(turning.map((entity) => entity.logicalFixtureId))],
		entities: turning,
		pivot: pivotOf(turning),
		pivotPage: gizmo.origin,
		startAngle: pageAngle(point, gizmo.origin),
		turn: 0,
	};
}

/** The turn a pointer at `point` asks for, snapped unless `free`. */
export function turnAt(
	context: Pick<CadViewportContext, "view" | "rotationQuarterTurns">,
	drag: RotateDrag,
	point: [number, number],
	free: boolean,
): number {
	const swept = pageAngle(point, drag.pivotPage) - drag.startAngle;
	return snappedTurn(swept * pageTurnSign(context.view, context.rotationQuarterTurns), free);
}

/** Where the selection stands and how it is turned for the drag's current turn. */
export function placementsFor(
	context: Pick<CadViewportContext, "view">,
	drag: RotateDrag,
): TurnedPlacement[] {
	return turnedPlacements(
		drag.entities,
		drag.fixtureIds,
		drag.pivot,
		rotationAxisIndex(context.view),
		drag.turn,
	);
}

/** The readout beside the gizmo while it turns: the axis and the angle, signed. */
export function rotateReadout(
	context: Pick<CadViewportContext, "view">,
	drag: RotateDrag,
): MoveReadout {
	const axis = ROTATION_AXIS_LABELS[rotationAxisIndex(context.view)];
	const sign = drag.turn > 0 ? "+" : drag.turn < 0 ? "−" : "";
	return {
		anchor: drag.pivotPage,
		title: "Rotation",
		coordinates: [
			{
				axis: "horizontal",
				label: `Rotation ${axis}`,
				value: `${sign}${Math.abs(drag.turn)}°`,
				active: true,
			},
		],
		entry: "",
		invalid: false,
	};
}

/**
 * The rotate handle of one viewport: `begin` takes a press on the arc, `turnTo` follows the
 * pointer, `finish` commits the turn on release, and `readout` is the angle shown while it turns.
 */
export function useRotateHandle(
	context: CadViewportContext,
	toPage: (client: [number, number]) => [number, number],
) {
	const [readout, setReadout] = useState<MoveReadout | null>(null);

	function begin(event: React.PointerEvent<HTMLCanvasElement>): Drag | null {
		if (event.button !== 0 || event.altKey) return null;
		const start: [number, number] = [event.clientX, event.clientY];
		const rotate = beginRotate(context, toPage(start));
		if (!rotate) return null;
		setReadout(rotateReadout(context, rotate));
		return { type: "rotate", start, last: start, axis: "plane", rotate };
	}

	/** Turns the selection to where the pointer is now about the handle's pivot. */
	function turnTo(active: Drag, client: [number, number], free: boolean) {
		const turning = active.rotate;
		if (!turning) return;
		turning.turn = turnAt(context, turning, toPage(client), free);
		context.onPreview({
			entityIds: turning.fixtureIds,
			deltaMillimetres: [0, 0, 0],
			spread: false,
			placements: placementsFor(context, turning),
		});
		setReadout(rotateReadout(context, turning));
	}

	/** Commits the turn the release leaves; a turn of nothing commits nothing. */
	async function finish(active: Drag, event: { clientX: number; clientY: number; shiftKey: boolean }) {
		// Shift may have been pressed or let go since the last move; the release decides.
		turnTo(active, [event.clientX, event.clientY], event.shiftKey);
		setReadout(null);
		const turning = active.rotate;
		if (!turning?.turn) return context.onPreview(null);
		await context.onTransforms?.(placementsFor(context, turning));
	}

	return { readout, begin, turnTo, finish, clear: () => setReadout(null) };
}
