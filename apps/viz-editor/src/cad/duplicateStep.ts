/**
 * How far, and which way, a copy stands from what it was duplicated from.
 *
 * A copy steps by the footprint of what was duplicated, not by a fixed distance, so it lands beside
 * the original with their edges touching however large the element is: a 500 mm step hides a copy
 * of a 2 x 1 m stage deck under its original and barely moves a copy of a 4 m truss.
 *
 * The step follows the axis the operator last moved that element along, so duplicating carries on
 * the direction they were already building in. Until an element has been moved, and for a
 * selection whose members disagree, the step goes along the view's right.
 */
import { entityBounds } from "./marqueeSelection";
import { type CadEntity, type CadViewDirection, planeDelta } from "./types";

/** The step for an element that has no footprint to measure, in millimetres. */
export const DUPLICATE_FALLBACK_MILLIMETRES = 500;

/** Which axis of the drawing plane an element was last dragged along: 0 across, 1 up the page. */
const lastAxis = new Map<string, 0 | 1>();

/** Forgets every remembered axis. For tests, and for leaving one show for another. */
export function forgetMoveAxes(): void {
	lastAxis.clear();
}

/**
 * Remembers which way a finished move went, per element.
 *
 * The delta is in show millimetres, so it is projected onto the drawing plane first: the operator
 * means "the way I dragged it on this page", not a world axis.
 */
export function rememberMoveAxis(
	ids: readonly string[],
	deltaMillimetres: readonly [number, number, number],
	view: CadViewDirection,
	rotationQuarterTurns: number,
): void {
	const across = planeDelta([1, 0], view, rotationQuarterTurns);
	const up = planeDelta([0, 1], view, rotationQuarterTurns);
	const dot = (axis: readonly number[]) =>
		Math.abs(
			deltaMillimetres[0] * axis[0] +
				deltaMillimetres[1] * axis[1] +
				deltaMillimetres[2] * axis[2],
		);
	const alongAcross = dot(across);
	const alongUp = dot(up);
	// A move that went nowhere in this plane says nothing about which way the operator is working.
	if (alongAcross < 1 && alongUp < 1) return;
	const axis = alongUp > alongAcross ? 1 : 0;
	for (const id of ids) lastAxis.set(id, axis);
}

/**
 * The step a copy of `ids` takes, in show millimetres.
 *
 * The distance is the whole selection's extent along the chosen axis, so several elements copied
 * together keep their arrangement and the group lands clear of itself rather than overlapping.
 */
export function duplicateStep(
	entities: readonly CadEntity[],
	ids: readonly string[],
	view: CadViewDirection,
	rotationQuarterTurns: number,
): [number, number, number] {
	const chosen = entities.filter((entity) => ids.includes(entity.logicalFixtureId));
	const axes = new Set(chosen.map((entity) => lastAxis.get(entity.logicalFixtureId) ?? 0));
	// One remembered direction to agree on, or the view's right.
	const axis = axes.size === 1 ? [...axes][0] : 0;
	let low = Number.POSITIVE_INFINITY;
	let high = Number.NEGATIVE_INFINITY;
	for (const entity of chosen) {
		const bounds = entityBounds(entity, view, rotationQuarterTurns);
		low = Math.min(low, bounds.minimum[axis]);
		high = Math.max(high, bounds.maximum[axis]);
	}
	const extent = Number.isFinite(high - low) ? Math.round(high - low) : 0;
	const distance = extent >= 1 ? extent : DUPLICATE_FALLBACK_MILLIMETRES;
	const local: [number, number] = axis === 0 ? [distance, 0] : [0, -distance];
	return planeDelta(local, view, rotationQuarterTurns);
}
