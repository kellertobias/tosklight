/** Feature-owned spatial models. Generated wire DTOs are decoded at the API boundary. */

export type ProjectionPreset =
	| "top"
	| "bottom"
	| "front"
	| "back"
	| "left"
	| "right";
export type RankDirection = "ascending" | "descending";
export type RadialDirection = "outward" | "inward";
export type RadarSweep = "clockwise" | "counter_clockwise";

export interface Position3d {
	x: number;
	y: number;
	z: number;
}

/**
 * How a Stage position becomes the pair the shape ranks on. Absent means `planar`, which is
 * what every projection stored before the other two kinds existed is.
 */
export type ProjectionKind = "planar" | "cylindrical" | "spherical";

/**
 * One position and one direction place every kind; nothing else is needed to derive the rest.
 *
 * `view_direction` is the viewing direction for planar, the central axis for cylindrical, and
 * the direction from the anchor to the centre of the spread for spherical. `rotation_degrees` is
 * the roll about it: the turn of the viewing plane for planar, the start angle around the axis
 * for cylindrical, and without effect for spherical.
 */
export interface SpatialProjection {
	anchor: Position3d;
	view_direction: Position3d;
	rotation_degrees: number;
	preset?: ProjectionPreset | null;
	kind?: ProjectionKind;
}

export type SpatialSelectionShape =
	| { type: "grid"; angle_degrees: number; direction: RankDirection }
	| {
			type: "radial";
			center_u: number;
			center_v: number;
			direction: RadialDirection;
	  }
	| {
			type: "radar";
			center_u: number;
			center_v: number;
			start_angle_degrees: number;
			sweep: RadarSweep;
	  };

export interface SpatialSelectionMapping {
	projection: SpatialProjection;
	shape: SpatialSelectionShape;
}

export type MappingProvenance =
	| { type: "none" }
	| { type: "local"; group_id: string }
	| { type: "inherited"; source_group_ids: string[] }
	| { type: "mixed_source_mappings" };

export interface ProjectedSpatialPosition {
	fixture_id: string;
	u: number | null;
	v: number | null;
}

export interface SpatialRank {
	fixture_id: string;
	rank: number;
}

export interface SpatialWarning {
	type: "missing_position";
	fixture_id: string;
}

export interface ResolvedSpatialMapping {
	source_order: string[];
	effective_mapping?: SpatialSelectionMapping | null;
	mapping_provenance: MappingProvenance;
	ordered_fixture_ids: string[];
	projected_positions: ProjectedSpatialPosition[];
	ranks: SpatialRank[];
	rank_count: number;
	warnings: SpatialWarning[];
}

export type DynamicSelectionShape =
	| SpatialSelectionShape
	| { type: "random"; seed: number };

export type DynamicProjectionStage =
	| { type: "inherit" }
	| { type: "replace"; value: SpatialProjection };

export type DynamicShapeStage =
	| { type: "inherit" }
	| { type: "replace"; value: DynamicSelectionShape };

export interface DynamicSpatialMapping {
	projection: DynamicProjectionStage;
	shape: DynamicShapeStage;
}
