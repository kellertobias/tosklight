/**
 * The typefaces CAD text can be set in: the screen's own, osifont for lettering to ISO 3098, and
 * three of Allen V. Hershey's plotter alphabets.
 *
 * Each ships unaltered in `public/fonts/cad` with its licence beside it (see `fonts.json` there).
 * A show stores only a typeface's ID, so it opens anywhere; an ID this build does not ship is drawn
 * in the screen's own. The Hershey alphabets have no umlauts, ß or several symbols, so a character
 * one of them lacks is drawn in osifont rather than in whatever the system has.
 */
import "./cadFonts.css";

export interface CadFont {
	/** Stored with the text; empty for the screen's own. */
	id: string;
	label: string;
	/** The CSS font-family the words are drawn with, or null for the screen's own. */
	family: string | null;
}

const FALLBACK = '"ToskLight CAD osifont", sans-serif';

export const CAD_FONTS: readonly CadFont[] = [
	{ id: "", label: "Screen (default)", family: null },
	{ id: "osifont", label: "ISO 3098 (osifont)", family: FALLBACK },
	{ id: "hershey-simplex", label: "Hershey Simplex", family: `"ToskLight CAD Hershey Simplex", ${FALLBACK}` },
	{ id: "hershey-duplex", label: "Hershey Duplex", family: `"ToskLight CAD Hershey Duplex", ${FALLBACK}` },
	{ id: "hershey-complex", label: "Hershey Complex", family: `"ToskLight CAD Hershey Complex", ${FALLBACK}` },
];

/** The CSS font-family for a stored typeface ID, or null to draw in the screen's own. */
export function cadFontFamily(id: string | undefined): string | null {
	return CAD_FONTS.find((font) => font.id === (id ?? ""))?.family ?? null;
}
