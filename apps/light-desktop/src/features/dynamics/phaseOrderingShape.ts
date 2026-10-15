import type {
	DynamicDefinitionProjection,
	DynamicLaneProjection,
	DynamicPhaseOrderingProjection,
	DynamicSelectionShapeProjection,
	DynamicTargetBindingProjection,
} from "../../api/types";
import { dynamicSpatialDraft } from "./dynamicSpatialDraft";

export const INHERIT_ORDERING = "inherit";

/** Old ordering variants retain their original runtime meaning until deliberately edited. */
export function shapeForOrdering(
	ordering: DynamicPhaseOrderingProjection,
): DynamicSelectionShapeProjection | null {
	switch (ordering.type) {
		case "spatial":
			return ordering.shape.type === "replace" ? ordering.shape.value : null;
		case "grid_linear":
			return { type: "grid", angle_degrees: ordering.angle_degrees, direction: "ascending" };
		case "radial_out":
		case "radial_in":
			return { type: "radial", center_u: ordering.center_x, center_v: ordering.center_z,
				direction: ordering.type === "radial_in" ? "inward" : "outward" };
		case "axial":
			return { type: "radar", center_u: ordering.center_x, center_v: ordering.center_z,
				start_angle_degrees: -180, sweep: "counter_clockwise" };
		case "random_each_loop":
			return { type: "random", seed: ordering.seed };
		default:
			return null;
	}
}

export function effectiveOrderingShape(
	ordering: DynamicPhaseOrderingProjection,
	spatialMapping: unknown,
) {
	if (ordering.type !== "selection") return shapeForOrdering(ordering);
	const shape = dynamicSpatialDraft(spatialMapping).shape;
	return shape.type === "replace" ? shape.value : null;
}

export function inheritsOrdering(
	spatialMapping: unknown,
	binding: DynamicTargetBindingProjection,
	ordering: DynamicPhaseOrderingProjection = { type: "selection" },
): boolean {
	if (binding.type !== "live_group") return false;
	if (ordering.type === "spatial") return ordering.shape.type === "inherit";
	if (ordering.type !== "selection") return false;
	return dynamicSpatialDraft(spatialMapping).shape.type === "inherit";
}

export function phaseOrderingChoice(
	ordering: DynamicPhaseOrderingProjection,
	spatialMapping: unknown,
	binding: DynamicTargetBindingProjection,
) {
	if (inheritsOrdering(spatialMapping, binding, ordering)) return INHERIT_ORDERING;
	return effectiveOrderingShape(ordering, spatialMapping)?.type ?? "source_order";
}

export function orderingWithShape(
	shape: DynamicSelectionShapeProjection,
): DynamicPhaseOrderingProjection {
	return { type: "spatial", shape: { type: "replace", value: shape } };
}

/** Parameter edits retain legacy coordinate and loop semantics; mode changes are explicit. */
export function orderingWithUpdatedShape(
	current: DynamicPhaseOrderingProjection,
	next: DynamicSelectionShapeProjection,
): DynamicPhaseOrderingProjection {
	if (current.type === "grid_linear" && next.type === "grid")
		return { ...current, angle_degrees: next.angle_degrees };
	if ((current.type === "radial_in" || current.type === "radial_out") && next.type === "radial")
		return { type: next.direction === "inward" ? "radial_in" : "radial_out",
			center_x: next.center_u, center_z: next.center_v };
	if (current.type === "axial" && next.type === "radar")
		return { ...current, center_x: next.center_u, center_z: next.center_v };
	if (current.type === "random_each_loop" && next.type === "random")
		return { ...current, seed: next.seed };
	return orderingWithShape(next);
}

export function phaseOrderingForChoice(
	choice: string,
	current: DynamicSelectionShapeProjection | null,
): DynamicPhaseOrderingProjection {
	if (choice === INHERIT_ORDERING) return { type: "spatial", shape: { type: "inherit" } };
	if (choice === "source_order") return { type: "source_order" };
	if (current?.type === choice) return orderingWithShape(current);
	switch (choice) {
		case "grid":
			return orderingWithShape({ type: "grid", angle_degrees: 0, direction: "ascending" });
		case "radial":
			return orderingWithShape({ type: "radial", center_u: 0, center_v: 0, direction: "outward" });
		case "radar":
			return orderingWithShape({ type: "radar", center_u: 0, center_v: 0, start_angle_degrees: 0, sweep: "clockwise" });
		case "random":
			return orderingWithShape({ type: "random", seed: 201 });
		default:
			return { type: "source_order" };
	}
}

export function selectedDynamicPhase(
	dynamic: DynamicDefinitionProjection,
	lane: DynamicLaneProjection | undefined,
) {
	return dynamic.phase_mode === "per_lane" ? (lane?.phase ?? dynamic.phase) : dynamic.phase;
}
