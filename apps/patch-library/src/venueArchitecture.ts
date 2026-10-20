/** Integer-millimetre venue construction carried unchanged through portable shows. */
export type VenuePlanPoint = { x: number; y: number };
export type VenueSlabPurpose = "roof" | "balcony";
export type VenueFootprint = "round" | "square" | "rectangular";
export type VenueTableShape = "round" | "rectangular";
export type VenueChairStyle = "black_metal" | "white" | "bar_stool";
export type VenueWindowShape = "rectangular" | "round" | "arched";
export type VenueDoorLeaves = "single" | "double";
export type VenueDoorSwing = "inward" | "outward";
export type VenueDoorHinge = "left" | "right";
export type WallOpeningKind =
	| { kind: "window"; shape: VenueWindowShape }
	| {
			kind: "door";
			leaves: VenueDoorLeaves;
			swing: VenueDoorSwing;
			hinge: VenueDoorHinge;
	  };
export interface WallOpening {
	id: string;
	offset_millimetres: number;
	sill_millimetres: number;
	width_millimetres: number;
	height_millimetres: number;
	opening: WallOpeningKind;
}
export type VenueArchitecture =
	| {
			kind: "wall";
			openings: WallOpening[];
			ceiling_height_millimetres?: number | null;
	  }
	| {
			kind: "slab";
			purpose: VenueSlabPurpose;
			outline_millimetres: VenuePlanPoint[];
			elevation_millimetres: number;
			thickness_millimetres: number;
	  }
	| { kind: "pillar"; shape: VenueFootprint }
	| { kind: "table"; shape: VenueTableShape }
	| { kind: "chair"; style: VenueChairStyle }
	| { kind: "bar" };
