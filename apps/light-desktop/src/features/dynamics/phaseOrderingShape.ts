import type {
	DynamicPhaseOrderingProjection,
	DynamicSelectionShapeProjection,
	DynamicTargetBindingProjection,
} from "../../api/types";
import { dynamicSpatialDraft } from "./dynamicSpatialDraft";

/**
 * The Projection places fixtures in a plane; the Phase ordering turns that plane into the
 * one-dimensional order each lamp takes its phase from. Both are stored, so an ordering
 * change has to keep the authoritative shape stage in step with it.
 */

export const INHERIT_ORDERING = "inherit";

/** The shape stage a chosen ordering means. Selection order uses no shape at all. */
export function shapeForOrdering(
	ordering: DynamicPhaseOrderingProjection,
): DynamicSelectionShapeProjection | null {
	switch (ordering.type) {
		case "grid_linear":
			return {
				type: "grid",
				angle_degrees: ordering.angle_degrees,
				direction: "ascending",
			};
		case "radial_out":
		case "radial_in":
			return {
				type: "radial",
				center_u: ordering.center_x,
				center_v: ordering.center_z,
				direction: ordering.type === "radial_in" ? "inward" : "outward",
			};
		case "axial":
			return {
				type: "radar",
				center_u: ordering.center_x,
				center_v: ordering.center_z,
				start_angle_degrees: 0,
				sweep: "clockwise",
			};
		case "random_each_loop":
			return { type: "random", seed: ordering.seed };
		default:
			return null;
	}
}

/**
 * Whether the ordering control should read as Inherit.
 *
 * A Dynamic with no stored spatial mapping is a legacy one whose ordering is authoritative,
 * and a Dynamic that is not bound to a live Group has nothing to inherit from; neither reads
 * as Inherit however its shape stage decodes.
 */
export function inheritsOrdering(
	spatialMapping: unknown,
	binding: DynamicTargetBindingProjection,
): boolean {
	if (binding.type !== "live_group") return false;
	if (!spatialMapping || typeof spatialMapping !== "object") return false;
	return dynamicSpatialDraft(spatialMapping).shape.type === "inherit";
}

/** The spatial mapping to store for an ordering choice, keeping the projection stage as it is. */
export function spatialMappingForOrdering(
	spatialMapping: unknown,
	ordering: DynamicPhaseOrderingProjection | null,
) {
	const current = dynamicSpatialDraft(spatialMapping);
	if (!ordering) return { ...current, shape: { type: "inherit" as const } };
	const shape = shapeForOrdering(ordering);
	return {
		...current,
		shape: shape
			? { type: "replace" as const, value: shape }
			: { type: "inherit" as const },
	};
}
