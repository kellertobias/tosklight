import type { VenueArchitecture } from "@tosklight/patch";

/** Name the authored construction, independent of the visual-only carrier profile. */
export function venueObjectLabel(architecture: VenueArchitecture): string {
	const kind =
		architecture.kind === "slab" ? architecture.purpose : architecture.kind;
	return kind.charAt(0).toUpperCase() + kind.slice(1);
}
