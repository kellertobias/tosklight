import type { WallOpening } from "@tosklight/patch";
import { viewDepth } from "./planGeometry";
import { type PlanPoint, rotateDeskPoint } from "./projection";
import { type CadEntity, type CadViewDirection, projectPoint } from "./types";
import { wallEndpoints } from "./venueShapes";
import { outlineVenueIds } from "./venueViewVisibility";

export type VenueDrawKind = "wall" | "window" | "door" | "pillar" | "stairs";
export const VENUE_DRAW_KINDS: readonly string[] = [
	"wall",
	"window",
	"door",
	"pillar",
	"stairs",
];
export function wallWorldPoint(
	wall: CadEntity,
	across: number,
	height = 0,
): [number, number, number] {
	const local = rotateDeskPoint([across, 0, height], wall.rotationDegrees);
	return local.map((value, axis) => value + wall.positionMillimetres[axis]) as [
		number,
		number,
		number,
	];
}
export function wallLocalPoint(
	wall: CadEntity,
	point: PlanPoint,
	view: CadViewDirection,
	turn: number,
) {
	const base = projectPoint(wall.positionMillimetres, view, turn);
	const across = projectPoint(
		rotateDeskPoint([1, 0, 0], wall.rotationDegrees),
		view,
		turn,
	);
	const up = projectPoint(
		rotateDeskPoint([0, 0, 1], wall.rotationDegrees),
		view,
		turn,
	);
	const dx = point[0] - base[0],
		dy = point[1] - base[1];
	const determinant = across[0] * up[1] - across[1] * up[0];
	if (view !== "top_down")
		return Math.abs(determinant) < 0.01
			? null
			: {
					across: (dx * up[1] - dy * up[0]) / determinant,
					height: (across[0] * dy - across[1] * dx) / determinant,
					distance: 0,
				};
	const square = across[0] ** 2 + across[1] ** 2;
	if (square < 0.001) return null;
	return {
		across: (dx * across[0] + dy * across[1]) / square,
		height: 0,
		distance: Math.abs(dx * across[1] - dy * across[0]) / Math.sqrt(square),
	};
}
export interface OpeningPreview {
	wall: CadEntity;
	opening: WallOpening;
	points: PlanPoint[];
	valid: boolean;
	reason: string | null;
}
export function openingPreview(
	entities: readonly CadEntity[],
	point: PlanPoint,
	kind: "window" | "door",
	view: CadViewDirection,
	turn: number,
	threshold: number,
): OpeningPreview | null {
	let best: OpeningPreview | null = null;
	let nearest = Infinity;
	for (const wall of [...entities].sort(
		(a, b) => viewDepth(b, view) - viewDepth(a, view),
	)) {
		if (!wall.selectable || wall.scenery?.architecture?.kind !== "wall")
			continue;
		const local = wallLocalPoint(wall, point, view, turn);
		if (!local) continue;
		const [length, thickness, height] = wall.sizeMillimetres;
		const distance = Math.max(
			local.distance - thickness / 2,
			Math.abs(local.across) - length / 2,
			view === "top_down" ? 0 : -local.height,
			view === "top_down" ? 0 : local.height - height,
			0,
		);
		if (distance > threshold || distance >= nearest) continue;
		const width = Math.min(kind === "door" ? 900 : 1000, length),
			openingHeight = Math.min(kind === "door" ? 2100 : 1000, height);
		const offset = Math.round(
			Math.max(
				0,
				Math.min(length - width, local.across + length / 2 - width / 2),
			),
		);
		const sill =
			kind === "door"
				? 0
				: Math.round(
						Math.max(
							0,
							Math.min(
								height - openingHeight,
								view === "top_down" ? 1000 : local.height - openingHeight / 2,
							),
						),
					);
		const opening: WallOpening = {
			id: "00000000-0000-4000-8000-000000000001",
			offset_millimetres: offset,
			sill_millimetres: sill,
			width_millimetres: Math.round(width),
			height_millimetres: Math.round(openingHeight),
			opening:
				kind === "door"
					? { kind: "door", leaves: "single", swing: "inward", hinge: "left" }
					: { kind: "window", shape: "rectangular" },
		};
		const overlaps = wall.scenery.architecture.openings.some(
			(other) =>
				offset < other.offset_millimetres + other.width_millimetres &&
				offset + width > other.offset_millimetres &&
				sill < other.sill_millimetres + other.height_millimetres &&
				sill + openingHeight > other.sill_millimetres,
		);
		const left = offset - length / 2;
		const points =
			view === "top_down"
				? [
						[left, -thickness / 2],
						[left + width, -thickness / 2],
						[left + width, thickness / 2],
						[left, thickness / 2],
					].map(([x, depth]) => {
						const world = rotateDeskPoint(
							[x, depth, 0],
							wall.rotationDegrees,
						).map((value, axis) => value + wall.positionMillimetres[axis]) as [
							number,
							number,
							number,
						];
						return projectPoint(world, view, turn);
					})
				: [
						[left, sill],
						[left + width, sill],
						[left + width, sill + openingHeight],
						[left, sill + openingHeight],
					].map(([x, z]) =>
						projectPoint(wallWorldPoint(wall, x, z), view, turn),
					);
		nearest = distance;
		best = {
			wall,
			opening,
			points,
			valid: !overlaps,
			reason: overlaps
				? "This opening overlaps an existing window or door."
				: null,
		};
	}
	return best;
}
/** Pick an opening through the aperture, so an operator can select its wall-owned settings. */
export function pickWallOpening(
	entities: readonly CadEntity[],
	point: PlanPoint,
	view: CadViewDirection,
	turn: number,
	threshold: number,
): { wall: CadEntity; opening: WallOpening } | null {
	const outlined = outlineVenueIds(entities, view);
	for (const wall of [...entities].sort(
		(a, b) => viewDepth(b, view) - viewDepth(a, view),
	)) {
		if (!wall.selectable || wall.scenery?.architecture?.kind !== "wall")
			continue;
		const local = wallLocalPoint(wall, point, view, turn);
		if (!local || local.distance > wall.sizeMillimetres[1] / 2 + threshold)
			continue;
		const x = local.across + wall.sizeMillimetres[0] / 2;
		for (const opening of wall.scenery.architecture.openings) {
			if (
				x < opening.offset_millimetres - threshold ||
				x > opening.offset_millimetres + opening.width_millimetres + threshold
			)
				continue;
			if (
				view !== "top_down" &&
				(local.height < opening.sill_millimetres - threshold ||
					local.height >
						opening.sill_millimetres + opening.height_millimetres + threshold)
			)
				continue;
			return { wall, opening };
		}
		// A nearer solid wall occludes an opening on a farther wall in an elevation.
		if (
			!outlined.has(wall.id) &&
			view !== "top_down" &&
			Math.abs(local.across) <= wall.sizeMillimetres[0] / 2 &&
			local.height >= 0 &&
			local.height <= wall.sizeMillimetres[2]
		)
			return null;
	}
	return null;
}
export function snapWallEndpoint(
	point: [number, number, number],
	entities: readonly CadEntity[],
	threshold: number,
): [number, number, number] {
	let result = point;
	let closest = threshold;
	for (const entity of entities)
		for (const endpoint of wallEndpoints(entity)) {
			if (Math.abs(endpoint[2] - point[2]) > 250) continue;
			const distance = Math.hypot(
				endpoint[0] - point[0],
				endpoint[1] - point[1],
			);
			if (distance <= closest) {
				closest = distance;
				result = endpoint;
			}
		}
	return result.map((value) => Math.round(value) + 0) as [
		number,
		number,
		number,
	];
}
