/**
 * The move gizmo's rotate handle: a quarter-circle arc between its two arrows that turns the
 * selection about the axis the view looks along — up in a plan, deep in a front or back view,
 * across in a side view.
 *
 * Turning writes the same rotation component Info's **Rotation** field for that axis sets, so the
 * handle, the typed field and the Visualizer agree on what a turn means. Several elements turn
 * together about the gizmo's pivot, each carried round it as well as turned. The drag snaps to
 * 15° steps; Shift, held, turns freely.
 */
import { rotateDeskPoint } from "./projection";
import type { CadEntity, CadViewDirection } from "./types";
import { projectPoint } from "./types";

type Vec3 = [number, number, number];

/** The handle's radius as a share of the arrows' length, and where along the quarter it runs. */
const ARC_RADIUS = 0.62;
const ARC_FROM_DEGREES = 12;
const ARC_TO_DEGREES = 78;
/** How far off the arc, in screen pixels, a press still takes it. */
const ARC_REACH_PIXELS = 8;
/** The steps a turn snaps to unless Shift is held. */
export const ROTATION_STEP_DEGREES = 15;

/** Which rotation component a view turns: the one about the axis it looks along. */
export function rotationAxisIndex(view: CadViewDirection): 0 | 1 | 2 {
	if (view === "top_down") return 2;
	if (view === "front_to_back" || view === "back_to_front") return 1;
	return 0;
}

export const ROTATION_AXIS_LABELS = ["X", "Y", "Z"] as const;

/**
 * Whether a positive turn of the view's component turns things anticlockwise on this page (1) or
 * clockwise (-1): read off the projection itself, so every view and page rotation agrees.
 */
export function pageTurnSign(view: CadViewDirection, rotationQuarterTurns: number): 1 | -1 {
	const axis = rotationAxisIndex(view);
	const turn: Vec3 = [0, 0, 0];
	turn[axis] = 10;
	// A point off the axis the view looks along, so it shows the turn on the page.
	const probe: Vec3 = axis === 2 ? [1000, 0, 0] : axis === 1 ? [1000, 0, 0] : [0, 1000, 0];
	const before = projectPoint(probe, view, rotationQuarterTurns);
	const after = projectPoint(rotateDeskPoint(probe, turn), view, rotationQuarterTurns);
	return before[0] * after[1] - before[1] * after[0] >= 0 ? 1 : -1;
}

/** The arc's points on the page about the gizmo's origin, for drawing it. */
export function rotateArc(origin: readonly [number, number], length: number): [number, number][] {
	const radius = length * ARC_RADIUS;
	return Array.from({ length: 13 }, (_, index) => {
		const degrees = ARC_FROM_DEGREES + ((ARC_TO_DEGREES - ARC_FROM_DEGREES) * index) / 12;
		const angle = (degrees * Math.PI) / 180;
		return [origin[0] + Math.cos(angle) * radius, origin[1] + Math.sin(angle) * radius];
	});
}

/** Whether a press on the page takes the arc. */
export function hitsRotateArc(
	point: readonly [number, number],
	origin: readonly [number, number],
	length: number,
	zoom: number,
): boolean {
	const dx = point[0] - origin[0];
	const dy = point[1] - origin[1];
	const off = Math.abs(Math.hypot(dx, dy) - length * ARC_RADIUS);
	const degrees = (Math.atan2(dy, dx) * 180) / Math.PI;
	return off <= ARC_REACH_PIXELS / zoom && degrees >= ARC_FROM_DEGREES - 6 && degrees <= ARC_TO_DEGREES + 6;
}

/** The page angle of a point about the pivot, in degrees. */
export function pageAngle(point: readonly [number, number], pivot: readonly [number, number]): number {
	return (Math.atan2(point[1] - pivot[1], point[0] - pivot[0]) * 180) / Math.PI;
}

/** A turn in degrees, into (-180, 180], snapped to the step unless it turns freely. */
export function snappedTurn(degrees: number, free: boolean): number {
	const wrapped = ((((degrees + 180) % 360) + 360) % 360) - 180;
	const turn = wrapped === -180 ? 180 : wrapped;
	return free ? Math.round(turn * 10) / 10 : Math.round(turn / ROTATION_STEP_DEGREES) * ROTATION_STEP_DEGREES;
}

export interface TurnedPlacement {
	id: string;
	positionMillimetres: Vec3;
	rotationDegrees: Vec3;
}

/**
 * Where each selected fixture stands and how it is turned after turning the selection `degrees`
 * about `pivot`, on the view's own rotation component: each is carried round the pivot and turned
 * by the same amount. Only a fixture's own placement turns; its multi-patch copies keep theirs.
 */
export function turnedPlacements(
	entities: readonly CadEntity[],
	fixtureIds: readonly string[],
	pivot: Vec3,
	axis: 0 | 1 | 2,
	degrees: number,
): TurnedPlacement[] {
	const turn: Vec3 = [0, 0, 0];
	turn[axis] = degrees;
	const wanted = new Set(fixtureIds);
	return entities
		.filter((entity) => wanted.has(entity.logicalFixtureId) && entity.id === entity.logicalFixtureId)
		.map((entity) => {
			const offset: Vec3 = [
				entity.positionMillimetres[0] - pivot[0],
				entity.positionMillimetres[1] - pivot[1],
				entity.positionMillimetres[2] - pivot[2],
			];
			const moved = rotateDeskPoint(offset, turn);
			const rotation: Vec3 = [...entity.rotationDegrees];
			const next = rotation[axis] + degrees;
			rotation[axis] = ((((next + 180) % 360) + 360) % 360) - 180;
			return {
				id: entity.id,
				positionMillimetres: moved.map((value, index) => Math.round(value + pivot[index])) as Vec3,
				rotationDegrees: rotation,
			};
		});
}

/** The entities a preview draws: each turned placement in place of where it stood. */
export function withTurnedPlacements(
	entities: readonly CadEntity[],
	placements: readonly TurnedPlacement[] | undefined,
): readonly CadEntity[] {
	if (!placements?.length) return entities;
	const byId = new Map(placements.map((placement) => [placement.id, placement]));
	return entities.map((entity) => {
		const placement = byId.get(entity.id);
		return placement
			? {
					...entity,
					positionMillimetres: placement.positionMillimetres,
					rotationDegrees: placement.rotationDegrees,
				}
			: entity;
	});
}
