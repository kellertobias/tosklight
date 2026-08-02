import type {
	DynamicSelectionShapeProjection,
	DynamicSpatialMappingOverrideProjection,
	DynamicSpatialProjectionStageProjection,
	DynamicSpatialShapeStageProjection,
	GroupMappingProjection,
} from "../../api/generated/light-wire";
import type { DynamicTargetBindingProjection } from "../../api/types";

export type DynamicMappingShape = DynamicSelectionShapeProjection;
export type DynamicSpatialMappingDraft =
	DynamicSpatialMappingOverrideProjection;

export const INHERIT_SPATIAL_MAPPING: DynamicSpatialMappingDraft = {
	projection: { type: "inherit" },
	shape: { type: "inherit" },
};

export function dynamicSpatialDraft(
	value: unknown,
): DynamicSpatialMappingDraft {
	if (!isRecord(value)) return structuredClone(INHERIT_SPATIAL_MAPPING);
	return {
		projection: projectionStage(value.projection),
		shape: shapeStage(value.shape),
	};
}

export function dynamicMappingBaseLabel(
	binding: DynamicTargetBindingProjection,
) {
	return binding.type === "live_group"
		? `Inherit group mapping · Group ${binding.group_id}`
		: "Selection order (no Group mapping)";
}

export function validateDynamicSpatialDraft(
	draft: DynamicSpatialMappingDraft,
	hasInheritedMapping: boolean,
) {
	const shapeOverride =
		draft.shape.type === "replace" ? draft.shape.value : null;
	if (shapeOverride?.type === "random") {
		return Number.isSafeInteger(shapeOverride.seed) && shapeOverride.seed >= 0
			? null
			: "Random seed must be a non-negative safe integer.";
	}
	if (draft.projection.type === "replace") {
		const projectionError = validateProjection(draft.projection.value);
		if (projectionError) return projectionError;
	}
	if (draft.shape.type === "replace") {
		const shapeError = validateShape(draft.shape.value);
		if (shapeError) return shapeError;
	}
	if (
		!hasInheritedMapping &&
		(draft.projection.type === "replace") !== (draft.shape.type === "replace")
	)
		return "Without a Group mapping, Projection and Shape must both be overridden or both use selection order.";
	return null;
}

export function sameDynamicSpatialDraft(
	left: DynamicSpatialMappingDraft,
	right: DynamicSpatialMappingDraft,
) {
	return JSON.stringify(left) === JSON.stringify(right);
}

function projectionStage(
	value: unknown,
): DynamicSpatialProjectionStageProjection {
	if (
		!isRecord(value) ||
		value.type !== "replace" ||
		!isProjection(value.value)
	)
		return { type: "inherit" };
	return { type: "replace", value: structuredClone(value.value) };
}

function shapeStage(value: unknown): DynamicSpatialShapeStageProjection {
	if (!isRecord(value) || value.type !== "replace" || !isShape(value.value))
		return { type: "inherit" };
	return { type: "replace", value: structuredClone(value.value) };
}

function validateProjection(projection: GroupMappingProjection) {
	const values = [
		projection.anchor.x,
		projection.anchor.y,
		projection.anchor.z,
		projection.view_direction.x,
		projection.view_direction.y,
		projection.view_direction.z,
		projection.rotation_degrees,
	];
	if (values.some((value) => !Number.isFinite(value)))
		return "Every Projection value must be finite.";
	if (
		Math.hypot(
			projection.view_direction.x,
			projection.view_direction.y,
			projection.view_direction.z,
		) <= 1e-12
	)
		return "View direction must not be zero.";
	return null;
}

function validateShape(shape: DynamicMappingShape) {
	if (shape.type === "random") return null;
	const values =
		shape.type === "grid"
			? [shape.angle_degrees]
			: shape.type === "radial"
				? [shape.center_u, shape.center_v]
				: [shape.center_u, shape.center_v, shape.start_angle_degrees];
	return values.some((value) => !Number.isFinite(value))
		? "Every Phaser value must be finite."
		: null;
}

function isProjection(value: unknown): value is GroupMappingProjection {
	if (!isRecord(value)) return false;
	return (
		isPosition(value.anchor) &&
		isPosition(value.view_direction) &&
		typeof value.rotation_degrees === "number"
	);
}

function isShape(value: unknown): value is DynamicMappingShape {
	if (!isRecord(value) || typeof value.type !== "string") return false;
	if (value.type === "grid")
		return (
			typeof value.angle_degrees === "number" &&
			(value.direction === "ascending" || value.direction === "descending")
		);
	if (value.type === "radial")
		return (
			typeof value.center_u === "number" &&
			typeof value.center_v === "number" &&
			(value.direction === "outward" || value.direction === "inward")
		);
	if (value.type === "radar")
		return (
			typeof value.center_u === "number" &&
			typeof value.center_v === "number" &&
			typeof value.start_angle_degrees === "number" &&
			(value.sweep === "clockwise" || value.sweep === "counter_clockwise")
		);
	return value.type === "random" && typeof value.seed === "number";
}

function isPosition(value: unknown) {
	return (
		isRecord(value) &&
		typeof value.x === "number" &&
		typeof value.y === "number" &&
		typeof value.z === "number"
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return Boolean(value && typeof value === "object" && !Array.isArray(value));
}
