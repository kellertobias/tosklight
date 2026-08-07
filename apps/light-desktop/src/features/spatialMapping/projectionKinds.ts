import type {
	Position3d,
	ProjectionKind,
	ProjectionPreset,
	SpatialProjection,
} from "./contracts";

export const PROJECTION_KINDS: ReadonlyArray<{
	value: ProjectionKind;
	label: string;
	detail: string;
}> = [
	{
		value: "planar",
		label: "Planar",
		detail:
			"Looks along the direction and ranks across the plane at right angles to it. Rotation turns that plane; the position is unused.",
	},
	{
		value: "cylindrical",
		label: "Cylindrical",
		detail:
			"The direction is the central axis through the position. The spread leaves the rotation angle both ways and meets itself 180° round.",
	},
	{
		value: "spherical",
		label: "Spherical",
		detail:
			"The direction points from the position to the centre of the spread, which reaches its opposite 180° away.",
	},
];

export const PROJECTION_PRESETS: ReadonlyArray<{
	value: ProjectionPreset;
	label: string;
}> = [
	{ value: "top", label: "Top" },
	{ value: "front", label: "Front" },
	{ value: "back", label: "Back" },
	{ value: "left", label: "Left" },
	{ value: "right", label: "Right" },
];

/** What a direction of nothing becomes, so a projection is never left unorientable. */
const FALLBACK_DIRECTION: Position3d = { x: 0, y: 0, z: -1 };

/** Absent means planar, so old Shows read as the kind they were stored as. */
export function projectionKind(projection: SpatialProjection): ProjectionKind {
	return projection.kind ?? "planar";
}

function hasDirection(direction: Position3d) {
	return direction.x !== 0 || direction.y !== 0 || direction.z !== 0;
}

/**
 * Switches kind while keeping the position and direction, because all three kinds are placed by
 * the same two. Only what each kind reads from them changes.
 */
export function withProjectionKind(
	projection: SpatialProjection,
	kind: ProjectionKind,
): SpatialProjection {
	if (projectionKind(projection) === kind) return projection;
	return {
		anchor: projection.anchor,
		view_direction: hasDirection(projection.view_direction)
			? projection.view_direction
			: FALLBACK_DIRECTION,
		rotation_degrees: projection.rotation_degrees,
		preset: null,
		kind,
	};
}

/**
 * The numeric fields one kind actually offers, ordered so the position comes first and the
 * orientation after it.
 *
 * Planar keeps its position in the data because the Radial and Radar shapes measure their
 * centres from it, but the projection itself does not read it, so it is not offered. A spherical
 * projection ranks by unsigned angle from its centre, which a roll about that centre does not
 * move, so it has no rotation to set.
 */
export function projectionFields(projection: SpatialProjection): ReadonlyArray<{
	key: string;
	label: string;
	value: number;
	unit?: string;
	apply(next: number): SpatialProjection;
}> {
	const kind = projectionKind(projection);
	const position = (axis: "x" | "y" | "z") => ({
		key: `position-${axis}`,
		label: `Position ${axis.toUpperCase()}`,
		value: projection.anchor[axis],
		apply: (next: number) => ({
			...projection,
			anchor: { ...projection.anchor, [axis]: next },
			preset: null,
		}),
	});
	const direction = (axis: "x" | "y" | "z") => ({
		key: `direction-${axis}`,
		label: `Direction ${axis.toUpperCase()}`,
		value: projection.view_direction[axis],
		apply: (next: number) => ({
			...projection,
			view_direction: { ...projection.view_direction, [axis]: next },
			preset: null,
		}),
	});
	const rotation = {
		key: "rotation",
		label: "Rotation",
		value: projection.rotation_degrees,
		unit: "°",
		apply: (next: number) => ({
			...projection,
			rotation_degrees: next,
			preset: null,
		}),
	};

	if (kind === "planar")
		return [direction("x"), direction("y"), direction("z"), rotation];

	const placed = [
		position("x"),
		position("y"),
		position("z"),
		direction("x"),
		direction("y"),
		direction("z"),
	];
	return kind === "cylindrical" ? [...placed, rotation] : placed;
}

/** Only a planar projection looks along a direction, so only it can carry a view preset. */
export function supportsPreset(projection: SpatialProjection) {
	return projectionKind(projection) === "planar";
}
