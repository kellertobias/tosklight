import artwork from "./assets/audience-outline.json";

export type AudienceOutlineView = "top" | "front" | "side";

export interface AudienceOutlineArtwork {
	source: string;
	top: [number, number][];
	front: [number, number][];
	side: [number, number][];
}

/**
 * Safe, flattened contours mechanically derived from the operator-supplied Illustrator SVG.
 * Coordinates are normalized by authored person height; the original vector sheet remains at
 * `assets/viz/crowd/Person Outline.svg` as the canonical artwork and provenance source.
 */
export const audienceOutline = artwork as AudienceOutlineArtwork;

export function audienceOutlineFor(view: AudienceOutlineView) {
	return audienceOutline[view];
}
