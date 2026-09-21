/**
 * Snapping in the CAD: a move or a measurement that comes close to a fit lands exactly on it.
 *
 * - A truss connector (the end of a straight run or of a corner piece's arm) onto another's.
 * - A stage element's corner onto another's, and its feet onto another stage element's top.
 * - A curtain's rail up under a truss or pipe, and its ends onto a neighbouring curtain's.
 * - A lamp's mounting point onto the nearest truss pipe.
 * - A measurement's ends onto any of those points, or an object's centre.
 *
 * Only the axes the drag can move are ever changed, and "close" is measured on those axes on
 * screen, so it feels the same at every zoom. A target far off the drag's plane — a truss 6 m above
 * a lamp on the floor, seen from above — is out of reach. Holding Shift turns snapping off.
 */
import type { PlanPoint } from "./projection";
import { type CadEntity, type CadViewDirection, projectPoint } from "./types";
import {
	addVec,
	boxCentre,
	closestOnSegment,
	curtainRail,
	isStageElement,
	mountingVolume,
	stageCorners,
	trussAxis,
	trussConnectors,
	trussPipeRadius,
	trussPipes,
	type Vec3,
} from "./venueShapes";

/** How close, in screen pixels, a feature must come to a target to snap onto it. */
export const SNAP_PIXELS = 12;

/** How far off the drag's plane a connector, pipe or rail may be and still be snapped to. */
export const SNAP_REACH_MILLIMETRES = 500;

/** The snap distance in plan millimetres at a camera zoom. */
export function snapThreshold(zoom: number): number {
	return Math.min(1000, Math.max(5, SNAP_PIXELS / zoom));
}

/** Which of x, y and z a drag can change. */
export type FreeAxes = readonly [boolean, boolean, boolean];

export interface MoveSnap {
	/** The move to make: the drag's own, corrected onto the fit when one is in reach. */
	delta: Vec3;
	/** Where the snapped features meet, in plan axes; empty when nothing snapped. */
	targets: Vec3[];
}

interface Candidate {
	correction: Vec3;
	distance: number;
	target: Vec3;
}

function pointCandidate(
	from: Vec3,
	to: Vec3,
	free: FreeAxes,
	threshold: number,
	reach: number | null,
): Candidate | null {
	const correction: Vec3 = [0, 0, 0];
	let squared = 0;
	for (const axis of [0, 1, 2] as const) {
		const gap = to[axis] - from[axis];
		if (free[axis]) {
			correction[axis] = gap;
			squared += gap * gap;
		} else if (reach !== null && Math.abs(gap) > reach) return null;
	}
	const distance = Math.sqrt(squared);
	return distance <= threshold ? { correction, distance, target: to } : null;
}

function footprint(corners: readonly Vec3[]) {
	const xs = corners.map((corner) => corner[0]);
	const ys = corners.map((corner) => corner[1]);
	return {
		min: [Math.min(...xs), Math.min(...ys)],
		max: [Math.max(...xs), Math.max(...ys)],
		bottom: Math.min(...corners.map((corner) => corner[2])),
		top: Math.max(...corners.map((corner) => corner[2])),
	};
}

/** The nearest fit for the moving features among the still ones. */
function nearestFit(
	movers: readonly CadEntity[],
	still: readonly CadEntity[],
	delta: Vec3,
	free: FreeAxes,
	threshold: number,
): Candidate | null {
	let best: Candidate | null = null;
	const consider = (candidate: Candidate | null) => {
		if (candidate && (!best || candidate.distance < best.distance)) best = candidate;
	};
	const moved = (point: readonly number[]) => addVec(point, delta);
	const connectors = still.flatMap(trussConnectors);
	const corners = still.filter(isStageElement).flatMap(stageCorners);
	const railEnds = still.flatMap((entity) => curtainRail(entity)?.ends ?? []);
	const pipes = still.flatMap(trussPipes);
	const axes = still.flatMap((entity) => trussAxis(entity) ?? []);
	const reach = SNAP_REACH_MILLIMETRES;
	for (const mover of movers) {
		for (const connector of trussConnectors(mover))
			for (const target of connectors)
				consider(pointCandidate(moved(connector), target, free, threshold, reach));
		if (isStageElement(mover))
			for (const corner of stageCorners(mover))
				for (const target of corners)
					consider(pointCandidate(moved(corner), target, free, threshold, null));
		const rail = curtainRail(mover);
		if (rail) {
			for (const end of rail.ends)
				for (const target of railEnds)
					consider(pointCandidate(moved(end), target, free, threshold, reach));
			const centre = moved(rail.centre);
			for (const axis of axes) {
				const on = closestOnSegment(centre, axis);
				consider(
					pointCandidate(centre, [on[0], on[1], on[2] - axis.halfSection], free, threshold, reach),
				);
			}
		}
	}
	return best;
}

/**
 * A lamp hanging from a truss pipe its clamp has reached across on the page.
 *
 * This is the rigging gesture: drag a lamp over a truss seen from above and it goes up onto the
 * pipe, however far below it started. So the test is an overlap in the plane the drag is happening
 * on — the clamp's footprint against the pipe's — and not a distance in the show, which would put
 * a truss at five metres permanently out of reach of a lamp on the floor. The correction then
 * carries every axis, including the one the drag itself cannot move, so the clamp meets the pipe
 * rather than the lamp's origin landing on the pipe's centre line.
 */
function nearestMount(
	movers: readonly CadEntity[],
	still: readonly CadEntity[],
	delta: Vec3,
	free: FreeAxes,
): Candidate | null {
	const pipes = still.flatMap((entity) => {
		const radius = trussPipeRadius(entity);
		return trussPipes(entity).map((segment) => ({ segment, radius }));
	});
	if (!pipes.length) return null;
	let best: Candidate | null = null;
	for (const mover of movers) {
		if (mover.kind === "venue") continue;
		const mount = mountingVolume(mover);
		if (!mount) continue;
		const centre = addVec(mount.centre, delta);
		const top = addVec(mount.top, delta);
		// How far the clamp reaches across the page, taking its widest side: a lamp turned on the
		// plan must not lose its grip because its narrow side happens to face the pipe.
		const reach = Math.max(
			...([0, 1, 2] as const).filter((axis) => free[axis]).map((axis) => mount.halfExtent[axis]),
			0,
		);
		for (const { segment, radius } of pipes) {
			const on = closestOnSegment(centre, segment);
			let squared = 0;
			for (const axis of [0, 1, 2] as const)
				if (free[axis]) squared += (on[axis] - centre[axis]) ** 2;
			const distance = Math.sqrt(squared);
			if (distance > reach + radius) continue;
			if (best && distance >= best.distance) continue;
			best = {
				correction: [on[0] - top[0], on[1] - top[1], on[2] - top[2]],
				distance,
				target: on,
			};
		}
	}
	return best;
}

/** A stage element's feet landing on another's top, when their footprints overlap. */
function nearestLanding(
	movers: readonly CadEntity[],
	still: readonly CadEntity[],
	delta: Vec3,
	threshold: number,
): Candidate | null {
	let best: Candidate | null = null;
	const decks = still.filter(isStageElement).map((entity) => footprint(stageCorners(entity)));
	for (const mover of movers.filter(isStageElement)) {
		const own = footprint(stageCorners(mover).map((corner) => addVec(corner, delta)));
		for (const deck of decks) {
			const overlaps =
				own.min[0] < deck.max[0] &&
				own.max[0] > deck.min[0] &&
				own.min[1] < deck.max[1] &&
				own.max[1] > deck.min[1];
			const gap = deck.top - own.bottom;
			if (!overlaps || Math.abs(gap) > threshold || (best && Math.abs(gap) >= best.distance))
				continue;
			best = {
				correction: [0, 0, gap],
				distance: Math.abs(gap),
				target: [
					(Math.max(own.min[0], deck.min[0]) + Math.min(own.max[0], deck.max[0])) / 2,
					(Math.max(own.min[1], deck.min[1]) + Math.min(own.max[1], deck.max[1])) / 2,
					deck.top,
				],
			};
		}
	}
	return best;
}

/**
 * The move `delta` of the objects `movingIds` names, snapped onto the nearest fit within
 * `threshold` millimetres on the `free` axes; the delta unchanged when nothing is in reach.
 */
export function snapMove(
	entities: readonly CadEntity[],
	movingIds: readonly string[],
	delta: readonly [number, number, number],
	free: FreeAxes,
	threshold: number,
): MoveSnap {
	const moving = new Set(movingIds);
	const movers = entities.filter((entity) => moving.has(entity.logicalFixtureId));
	const still = entities.filter((entity) => !moving.has(entity.logicalFixtureId));
	const start: Vec3 = [...delta];
	// Hanging a lamp on a pipe beats every other fit: it is the one the operator is reaching for,
	// and it is the only one that may move an axis the drag itself cannot.
	const mount = nearestMount(movers, still, start, free);
	if (mount)
		return {
			delta: addVec(start, mount.correction),
			targets: [mount.target],
		};
	const fit = nearestFit(movers, still, start, free, threshold);
	const landing = free[2] ? nearestLanding(movers, still, start, threshold) : null;
	const chosen = [fit, landing].filter((candidate): candidate is Candidate =>
		Boolean(candidate),
	);
	// A landing only adds height, so it joins a fit that left the height alone.
	const applied =
		fit && landing && Math.abs(fit.correction[2]) > 0.5 ? [fit] : chosen;
	return {
		delta: applied.reduce((sum, candidate) => addVec(sum, candidate.correction), start),
		targets: applied.map((candidate) => candidate.target),
	};
}

/** The points a measurement can snap to: connectors, stage corners, rail ends and every object's centre. */
function measurePoints(entities: readonly CadEntity[]): Vec3[] {
	return entities.flatMap((entity) => [
		...trussConnectors(entity),
		...(isStageElement(entity) ? stageCorners(entity) : []),
		...(curtainRail(entity)?.ends ?? []),
		boxCentre(entity),
	]);
}

/** A point put on a tile, snapped onto the nearest snap point within `threshold`; null when none is. */
export function snapPlanPoint(
	entities: readonly CadEntity[],
	point: PlanPoint,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	threshold: number,
): PlanPoint | null {
	let best: { point: PlanPoint; distance: number } | null = null;
	for (const candidate of measurePoints(entities)) {
		const projected = projectPoint(candidate, view, rotationQuarterTurns);
		const distance = Math.hypot(projected[0] - point[0], projected[1] - point[1]);
		if (distance <= threshold && (!best || distance < best.distance))
			best = { point: projected, distance };
	}
	return best?.point ?? null;
}
