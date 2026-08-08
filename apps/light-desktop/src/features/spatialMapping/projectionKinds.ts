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
			"The direction points from the position to the centre of the spread, which reaches its opposite 180° away. Rotation turns the meridian the spread is measured around.",
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

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

/**
 * A direction read as the two turns that aim it: azimuth swings it about world Z, elevation
 * lifts it off the horizon.
 *
 * Three independent components can aim a direction, but no operator turns an encoder thinking
 * in components. Angles are what the hand does — swing, then tilt — and they are what the
 * cylinder's axis and the sphere's centre are actually set by.
 */
export function directionAngles(direction: Position3d) {
	const length = Math.hypot(direction.x, direction.y, direction.z);
	if (length <= 1e-12) return { azimuth: 0, elevation: -90 };
	return {
		azimuth: (Math.atan2(direction.y, direction.x) * 180) / Math.PI,
		elevation: (Math.asin(clamp(direction.z / length, -1, 1)) * 180) / Math.PI,
	};
}

/** The inverse, matching the engine's own spherical frame. */
export function directionFromAngles(
	azimuth: number,
	elevation: number,
): Position3d {
	const swing = (azimuth * Math.PI) / 180;
	const lift = (elevation * Math.PI) / 180;
	return {
		x: Math.cos(lift) * Math.cos(swing),
		y: Math.cos(lift) * Math.sin(swing),
		z: Math.sin(lift),
	};
}

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
 * centres from it, but the projection itself does not read it, so it is not offered.
 *
 * The two placed kinds are aimed by angle rather than by direction components: azimuth swings
 * the axis, elevation pivots it, and Rotation is the last turn about it — where the spread
 * starts. Planar keeps its components, because a view preset names one and the numbers are how
 * a preset reads back.
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

	const angles = directionAngles(projection.view_direction);
	const aim = (which: "azimuth" | "elevation") => ({
		key: which,
		label: which === "azimuth" ? "Azimuth" : "Elevation",
		value: angles[which],
		unit: "°",
		apply: (next: number) => ({
			...projection,
			view_direction: directionFromAngles(
				which === "azimuth" ? next : angles.azimuth,
				which === "elevation" ? next : angles.elevation,
			),
			preset: null,
		}),
	});
	return [
		position("x"),
		position("y"),
		position("z"),
		aim("azimuth"),
		aim("elevation"),
		rotation,
	];
}

/** Only a planar projection looks along a direction, so only it can carry a view preset. */
export function supportsPreset(projection: SpatialProjection) {
	return projectionKind(projection) === "planar";
}
