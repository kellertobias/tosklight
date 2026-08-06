import type {
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
		detail: "Looks along one direction and ranks across the viewing plane.",
	},
	{
		value: "cylindrical",
		label: "Cylindrical",
		detail:
			"Spreads around an axis, outward from the start angle to 180° on the far side.",
	},
	{
		value: "spherical",
		label: "Spherical",
		detail: "Spreads outward from one point on the sphere to its opposite.",
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

const ZERO = { x: 0, y: 0, z: 0 };

/** Absent means planar, so old Shows read as the kind they were stored as. */
export function projectionKind(projection: SpatialProjection): ProjectionKind {
	return projection.kind ?? "planar";
}

export function axisRotation(projection: SpatialProjection) {
	return projection.axis_rotation ?? ZERO;
}

/**
 * Switches kind while keeping what the two kinds share. The anchor is the centre point for
 * the two angular kinds and the plane origin for planar, so it always carries over.
 */
export function withProjectionKind(
	projection: SpatialProjection,
	kind: ProjectionKind,
): SpatialProjection {
	if (projectionKind(projection) === kind) return projection;
	const base = { ...projection, kind, preset: null };
	if (kind === "planar")
		return {
			anchor: base.anchor,
			view_direction: base.view_direction,
			rotation_degrees: base.rotation_degrees,
			preset: null,
			kind,
		};
	return {
		...base,
		axis_rotation: axisRotation(projection),
		start_angle_degrees: projection.start_angle_degrees ?? 0,
		elevation_degrees: projection.elevation_degrees ?? 0,
	};
}

/**
 * The numeric fields one kind actually uses. Planar keeps its anchor in the data because the
 * Radial and Radar shapes measure their centres from it, but the operator sets only a
 * direction, so the anchor is not offered here.
 */
export function projectionFields(projection: SpatialProjection): ReadonlyArray<{
	key: string;
	label: string;
	value: number;
	unit?: string;
	apply(next: number): SpatialProjection;
}> {
	const kind = projectionKind(projection);
	const position = (
		axis: "x" | "y" | "z",
	): {
		key: string;
		label: string;
		value: number;
		apply(next: number): SpatialProjection;
	} => ({
		key: `position-${axis}`,
		label: `Position ${axis.toUpperCase()}`,
		value: projection.anchor[axis],
		apply: (next) => ({
			...projection,
			anchor: { ...projection.anchor, [axis]: next },
			preset: null,
		}),
	});
	const rotation = (axis: "x" | "y" | "z") => ({
		key: `rotation-${axis}`,
		label: `Rotation ${axis.toUpperCase()}`,
		value: axisRotation(projection)[axis],
		unit: "°",
		apply: (next: number) => ({
			...projection,
			axis_rotation: { ...axisRotation(projection), [axis]: next },
		}),
	});

	if (kind === "planar")
		return [
			{
				key: "direction-x",
				label: "Direction X",
				value: projection.view_direction.x,
				apply: (next) => ({
					...projection,
					view_direction: { ...projection.view_direction, x: next },
					preset: null,
				}),
			},
			{
				key: "direction-y",
				label: "Direction Y",
				value: projection.view_direction.y,
				apply: (next) => ({
					...projection,
					view_direction: { ...projection.view_direction, y: next },
					preset: null,
				}),
			},
			{
				key: "direction-z",
				label: "Direction Z",
				value: projection.view_direction.z,
				apply: (next) => ({
					...projection,
					view_direction: { ...projection.view_direction, z: next },
					preset: null,
				}),
			},
			{
				key: "rotation",
				label: "Rotation",
				value: projection.rotation_degrees,
				unit: "°",
				apply: (next) => ({
					...projection,
					rotation_degrees: next,
					preset: null,
				}),
			},
		];

	if (kind === "cylindrical")
		return [
			position("x"),
			position("y"),
			position("z"),
			rotation("x"),
			rotation("y"),
			rotation("z"),
			{
				key: "start-angle",
				label: "Start angle",
				value: projection.start_angle_degrees ?? 0,
				unit: "°",
				apply: (next) => ({ ...projection, start_angle_degrees: next }),
			},
		];

	return [
		position("x"),
		position("y"),
		position("z"),
		{
			key: "start-angle",
			label: "Centre azimuth",
			value: projection.start_angle_degrees ?? 0,
			unit: "°",
			apply: (next) => ({ ...projection, start_angle_degrees: next }),
		},
		{
			key: "elevation",
			label: "Centre elevation",
			value: projection.elevation_degrees ?? 0,
			unit: "°",
			apply: (next) => ({ ...projection, elevation_degrees: next }),
		},
	];
}

/** Only a planar projection looks along a direction, so only it can carry a view preset. */
export function supportsPreset(projection: SpatialProjection) {
	return projectionKind(projection) === "planar";
}
