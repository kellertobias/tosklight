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
	/** The font file in `public/fonts/cad`, which a printed plan embeds; none for the screen's own. */
	file: string | null;
}

/** The typeface a character another one lacks is drawn in, on screen and in print. */
export const CAD_FALLBACK_FONT_ID = "osifont";

const FALLBACK = '"ToskLight CAD osifont", sans-serif';

export const CAD_FONTS: readonly CadFont[] = [
	{ id: "", label: "Screen (default)", family: null, file: null },
	{ id: "osifont", label: "ISO 3098 (osifont)", family: FALLBACK, file: "osifont-lgpl3fe.ttf" },
	{
		id: "hershey-simplex",
		label: "Hershey Simplex",
		family: `"ToskLight CAD Hershey Simplex", ${FALLBACK}`,
		file: "AVHersheySimplexMedium.otf",
	},
	{
		id: "hershey-duplex",
		label: "Hershey Duplex",
		family: `"ToskLight CAD Hershey Duplex", ${FALLBACK}`,
		file: "AVHersheyDuplexMedium.otf",
	},
	{
		id: "hershey-complex",
		label: "Hershey Complex",
		family: `"ToskLight CAD Hershey Complex", ${FALLBACK}`,
		file: "AVHersheyComplexMedium.otf",
	},
];

/** The CSS font-family for a stored typeface ID, or null to draw in the screen's own. */
export function cadFontFamily(id: string | undefined): string | null {
	return CAD_FONTS.find((font) => font.id === (id ?? ""))?.family ?? null;
}
