/**
 * The shapes of Venue objects as the CAD reasons about them: where a box really sits, where a
 * truss's connectors and pipes are, a stage element's corners and a curtain's rail.
 *
 * A stage element — a deck on a scissor lift, stairs, or a shipped deck on fixed legs — is placed by
 * its feet: its position is the middle of its footprint on the floor it stands on, and its box
 * reaches `height` up from there. Every other object is placed by the centre of its box, except a
 * truss corner piece, which is placed by its corner node.
 */
import { rotateDeskPoint } from "./projection";
import { chordOffsets, trussParts } from "./trussPlan";
import type { CadEntity } from "./types";

export type Vec3 = [number, number, number];

export interface Segment {
	start: Vec3;
	end: Vec3;
}

type Shape = Pick<CadEntity, "positionMillimetres" | "rotationDegrees" | "sizeMillimetres"> &
	Partial<Pick<CadEntity, "scenery" | "fixtureProfile">>;

/** How far a shipped truss corner piece's arms reach from its node to the connector on their end. */
export const TRUSS_CORNER_ARM_MILLIMETRES = 500;

/** A deck on a scissor lift, stairs, or a shipped deck on fixed legs. */
export function isStageElement(entity: Shape): boolean {
	return (
		entity.scenery?.kind === "riser" ||
		/^Venue Stage (Deck|Element|Stairs)\b/iu.test(entity.fixtureProfile ?? "")
	);
}

/** Whether the object's position is the floor under it rather than the centre of its box. */
export function standsOnItsFeet(entity: Shape): boolean {
	return isStageElement(entity);
}

export function isTruss(entity: Shape): boolean {
	return entity.scenery?.kind === "truss";
}

export function isCurtain(entity: Shape): boolean {
	return entity.scenery?.kind === "curtain";
}

export function addVec(a: readonly number[], b: readonly number[]): Vec3 {
	return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

/** A point in the object's own axes (x across, y deep, z up) turned and moved into the plan. */
export function placedPoint(entity: Shape, local: Vec3): Vec3 {
	return addVec(entity.positionMillimetres, rotateDeskPoint(local, entity.rotationDegrees));
}

/** The centre of the object's box in plan axes. */
export function boxCentre(entity: Shape): Vec3 {
	return standsOnItsFeet(entity)
		? placedPoint(entity, [0, 0, entity.sizeMillimetres[2] / 2])
		: [...entity.positionMillimetres];
}

const ARMS: Readonly<Record<string, readonly Vec3[]>> = (() => {
	const left: Vec3 = [-1, 0, 0];
	const right: Vec3 = [1, 0, 0];
	const back: Vec3 = [0, 1, 0];
	const front: Vec3 = [0, -1, 0];
	const up: Vec3 = [0, 0, 1];
	const down: Vec3 = [0, 0, -1];
	return {
		"corner 2-way": [left, back],
		"t-piece 3-way": [left, right, back],
		"corner 3-way down": [left, back, down],
		"cross 4-way": [left, right, front, back],
		"t-piece 4-way down": [left, right, back, down],
		"cross 5-way down": [left, right, front, back, down],
		"node 6-way": [left, right, front, back, up, down],
	};
})();

/** The arm directions of a shipped three- or four-point truss corner piece, in its own axes. */
export function trussCornerArms(entity: Shape): readonly Vec3[] | null {
	const match = /^Venue (?:Three|Four)-Point Truss (.+)$/iu.exec(entity.fixtureProfile ?? "");
	return match ? (ARMS[match[1].trim().toLowerCase()] ?? null) : null;
}

/** A straight truss's run: which of its own axes it is long along, and its section. */
function trussRun(entity: Shape) {
	const size = entity.sizeMillimetres;
	const run = size.indexOf(Math.max(...size));
	const [across, up] = run === 0 ? [1, 2] : run === 1 ? [0, 2] : [0, 1];
	return { run, across, up, length: size[run], section: Math.max(size[across], size[up]) };
}

/**
 * Where a truss part is joined to the next: both ends of a straight truss's run, and the end of
 * every arm of a corner piece. The conical coupler ("egg") is centred on each of these points.
 */
export function trussConnectors(entity: Shape): Vec3[] {
	const arms = trussCornerArms(entity);
	if (arms)
		return arms.map((arm) =>
			placedPoint(entity, arm.map((value) => value * TRUSS_CORNER_ARM_MILLIMETRES) as Vec3),
		);
	if (!isTruss(entity)) return [];
	const { run, length } = trussRun(entity);
	return [-1, 1].map((sign) => {
		const local: Vec3 = [0, 0, 0];
		local[run] = (sign * length) / 2;
		return placedPoint(entity, local);
	});
}

/** Every pipe of a straight truss, as the plan draws its chords: what a clamp hangs from. */
export function trussPipes(entity: Shape): Segment[] {
	if (!isTruss(entity)) return [];
	const { run, across, up, length, section } = trussRun(entity);
	const parts = trussParts(section, entity.scenery?.chords ?? 1);
	return chordOffsets(parts).map(([sideways, height]) => {
		const at = (along: number) => {
			const local: Vec3 = [0, 0, 0];
			local[run] = along;
			local[across] = sideways;
			local[up] = height;
			return placedPoint(entity, local);
		};
		return { start: at(-length / 2), end: at(length / 2) };
	});
}

/** A straight truss's centre line and half its outside section: a curtain's rail hangs just under it. */
export function trussAxis(entity: Shape): (Segment & { halfSection: number }) | null {
	if (!isTruss(entity)) return null;
	const { run, length, section } = trussRun(entity);
	const at = (along: number) => {
		const local: Vec3 = [0, 0, 0];
		local[run] = along;
		return placedPoint(entity, local);
	};
	return { start: at(-length / 2), end: at(length / 2), halfSection: section / 2 };
}

/** The eight corners of a stage element's box, from the floor it stands on to its top. */
export function stageCorners(entity: Shape): Vec3[] {
	const [width, depth, height] = entity.sizeMillimetres;
	const bottom = standsOnItsFeet(entity) ? 0 : -height / 2;
	const corners: Vec3[] = [];
	for (const z of [bottom, bottom + height])
		for (const x of [-width / 2, width / 2])
			for (const y of [-depth / 2, depth / 2]) corners.push(placedPoint(entity, [x, y, z]));
	return corners;
}

/** A curtain's rail along its top edge: the middle of it and both ends. */
export function curtainRail(entity: Shape): { centre: Vec3; ends: [Vec3, Vec3] } | null {
	if (!isCurtain(entity)) return null;
	const [width, , height] = entity.sizeMillimetres;
	return {
		centre: placedPoint(entity, [0, 0, height / 2]),
		ends: [placedPoint(entity, [-width / 2, 0, height / 2]), placedPoint(entity, [width / 2, 0, height / 2])],
	};
}

/** The nearest point to `point` on a segment. */
export function closestOnSegment(point: readonly number[], segment: Segment): Vec3 {
	const direction = [0, 1, 2].map((axis) => segment.end[axis] - segment.start[axis]);
	const lengthSquared = direction.reduce((sum, value) => sum + value * value, 0);
	const t =
		lengthSquared > 0
			? Math.max(
					0,
					Math.min(
						1,
						[0, 1, 2].reduce(
							(sum, axis) => sum + (point[axis] - segment.start[axis]) * direction[axis],
							0,
						) / lengthSquared,
					),
				)
			: 0;
	return [0, 1, 2].map((axis) => segment.start[axis] + direction[axis] * t) as Vec3;
}
