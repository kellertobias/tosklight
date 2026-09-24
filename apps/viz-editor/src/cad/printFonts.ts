/**
 * The CAD text typefaces in a printed plan: each font a page's text is set in is embedded whole,
 * so the PDF reads the same on any computer as the plan does on screen.
 *
 * Each embeds as a composite (Type0) font addressed by glyph index — a TrueType program as
 * `FontFile2`, an OpenType program with CFF outlines as `FontFile3` — with every glyph's advance and
 * a ToUnicode map, so the words can be searched and copied. A character a Hershey alphabet lacks
 * (umlauts, ß) is set in osifont, as on screen; a character no embedded font has is set as `-`,
 * as the plan's own Helvetica does. Text in the screen's own typeface stays in Helvetica.
 */
import { CAD_FALLBACK_FONT_ID, CAD_FONTS } from "./cadFonts";
import { n } from "./printPdfOps";

/** What a PDF needs to know about one font program. */
export interface ParsedFont {
	bytes: Uint8Array;
	/** CFF outlines (an `OTTO` file) rather than TrueType ones. */
	cff: boolean;
	postscriptName: string;
	unitsPerEm: number;
	bbox: [number, number, number, number];
	ascent: number;
	descent: number;
	/** Glyph index for a code point, 0 when the font has none. */
	glyph(codePoint: number): number;
	/** A glyph's advance in font units. */
	advance(glyph: number): number;
}

/** Reads the tables a PDF needs from a TrueType or OpenType font. */
export function parseFont(bytes: Uint8Array): ParsedFont {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const u16 = (at: number) => view.getUint16(at);
	const i16 = (at: number) => view.getInt16(at);
	const u32 = (at: number) => view.getUint32(at);
	const tables = new Map<string, number>();
	for (let index = 0; index < u16(4); index++) {
		const at = 12 + index * 16;
		tables.set(String.fromCharCode(...bytes.subarray(at, at + 4)), u32(at + 8));
	}
	const table = (tag: string) => {
		const at = tables.get(tag);
		if (at === undefined) throw new Error(`The font has no ${tag} table`);
		return at;
	};
	const head = table("head");
	const hhea = table("hhea");
	const hmtx = table("hmtx");
	const metrics = u16(hhea + 34);
	const codes = cmapOf(bytes, view, table("cmap"));
	return {
		bytes,
		cff: u32(0) === 0x4f54544f,
		postscriptName: postscriptNameOf(bytes, view, table("name")),
		unitsPerEm: u16(head + 18),
		bbox: [i16(head + 36), i16(head + 38), i16(head + 40), i16(head + 42)],
		ascent: i16(hhea + 4),
		descent: i16(hhea + 6),
		glyph: (codePoint) => codes.get(codePoint) ?? 0,
		advance: (glyph) => u16(hmtx + 4 * Math.min(glyph, metrics - 1)),
	};
}

/** The Unicode map of a `cmap` table: formats 4 and 12, which every shipped font carries. */
function cmapOf(bytes: Uint8Array, view: DataView, at: number): Map<number, number> {
	const codes = new Map<number, number>();
	for (let index = 0; index < view.getUint16(at + 2); index++) {
		const platform = view.getUint16(at + 4 + index * 8);
		if (platform !== 0 && platform !== 3) continue;
		const table = at + view.getUint32(at + 8 + index * 8);
		const format = view.getUint16(table);
		if (format === 4) {
			const segments = view.getUint16(table + 6) / 2;
			const ends = table + 14;
			const starts = ends + segments * 2 + 2;
			const deltas = starts + segments * 2;
			const offsets = deltas + segments * 2;
			for (let segment = 0; segment < segments; segment++) {
				const start = view.getUint16(starts + segment * 2);
				const end = view.getUint16(ends + segment * 2);
				const delta = view.getInt16(deltas + segment * 2);
				const offset = view.getUint16(offsets + segment * 2);
				for (let code = start; code <= end && code !== 0xffff; code++) {
					let glyph = 0;
					if (!offset) glyph = (code + delta) & 0xffff;
					else {
						const slot = offsets + segment * 2 + offset + (code - start) * 2;
						const raw = slot + 2 <= bytes.length ? view.getUint16(slot) : 0;
						glyph = raw ? (raw + delta) & 0xffff : 0;
					}
					if (glyph && !codes.has(code)) codes.set(code, glyph);
				}
			}
		} else if (format === 12) {
			for (let group = 0; group < view.getUint32(table + 12); group++) {
				const at = table + 16 + group * 12;
				const start = view.getUint32(at);
				for (let code = start; code <= view.getUint32(at + 4); code++)
					if (!codes.has(code)) codes.set(code, view.getUint32(at + 8) + code - start);
			}
		}
	}
	return codes;
}

/** The font's PostScript name (name ID 6), kept to the characters a PDF name allows. */
function postscriptNameOf(bytes: Uint8Array, view: DataView, at: number): string {
	const strings = at + view.getUint16(at + 4);
	for (let index = 0; index < view.getUint16(at + 2); index++) {
		const record = at + 6 + index * 12;
		if (view.getUint16(record + 6) !== 6) continue;
		const start = strings + view.getUint16(record + 10);
		const raw = bytes.subarray(start, start + view.getUint16(record + 8));
		const text =
			view.getUint16(record) === 1
				? String.fromCharCode(...raw)
				: String.fromCharCode(...Array.from({ length: raw.length / 2 }, (_, i) => (raw[i * 2] << 8) | raw[i * 2 + 1]));
		const name = text.replace(/[^A-Za-z0-9-]/g, "");
		if (name) return name;
	}
	return "ToskLightCADFont";
}

/**
 * The fonts a document may set text in, and every glyph it has set in each — which is what the
 * document's font objects list. One set serves one document.
 */
export class PrintFontSet {
	private readonly used = new Map<string, Map<number, number>>();

	constructor(private readonly fonts: ReadonlyMap<string, ParsedFont>) {}

	/**
	 * Content-stream text in the typeface `fontId`, starting at (x, y); null when that typeface is
	 * not embedded, so the caller sets it in Helvetica.
	 */
	text(value: string, fontId: string | undefined, x: number, y: number, size: number): string | null {
		const primary = fontId ? this.fonts.get(fontId) : undefined;
		if (!primary || !fontId) return null;
		const fallback = this.fonts.get(CAD_FALLBACK_FONT_ID);
		const runs: Array<{ id: string; glyphs: number[] }> = [];
		for (const character of value) {
			let code = character.codePointAt(0) ?? 0x2d;
			let id = fontId;
			let glyph = primary.glyph(code);
			if (!glyph && fallback?.glyph(code)) {
				id = CAD_FALLBACK_FONT_ID;
				glyph = fallback.glyph(code);
			}
			// A character neither font has prints as a dash, as the plan's Helvetica prints it.
			if (!glyph) {
				code = 0x2d;
				glyph = primary.glyph(code);
			}
			this.record(id, glyph, code);
			const last = runs.at(-1);
			if (last?.id === id) last.glyphs.push(glyph);
			else runs.push({ id, glyphs: [glyph] });
		}
		const shown = runs.map(
			({ id, glyphs }) =>
				`/${resourceName(id)} ${n(size)} Tf <${glyphs.map((glyph) => glyph.toString(16).padStart(4, "0")).join("")}> Tj`,
		);
		return `BT ${n(x)} ${n(y)} Td ${shown.join(" ")} ET`;
	}

	private record(id: string, glyph: number, code: number) {
		const glyphs = this.used.get(id) ?? new Map<number, number>();
		if (!glyphs.has(glyph)) glyphs.set(glyph, code);
		this.used.set(id, glyphs);
	}

	/** The PDF objects of every font the document has set text in, numbered from `firstId`. */
	objects(firstId: number): { resources: string; objects: string[] } {
		const resources: string[] = [];
		const objects: string[] = [];
		for (const [id, glyphs] of this.used) {
			const font = this.fonts.get(id);
			if (!font) continue;
			const base = firstId + objects.length;
			resources.push(`/${resourceName(id)} ${base} 0 R`);
			objects.push(...fontObjects(font, glyphs, base));
		}
		return { resources: resources.join(" "), objects };
	}
}

/** The name a page's resources give a CAD typeface. */
function resourceName(id: string): string {
	return `CAD-${id}`;
}

/** The five objects of one embedded font, the first at `base`: Type0, CIDFont, descriptor, program, ToUnicode. */
function fontObjects(font: ParsedFont, glyphs: ReadonlyMap<number, number>, base: number): string[] {
	const scale = 1000 / font.unitsPerEm;
	const name = font.postscriptName;
	const widths = [...glyphs.keys()]
		.sort((a, b) => a - b)
		.map((glyph) => `${glyph} [${Math.round(font.advance(glyph) * scale)}]`)
		.join(" ");
	const program = hex(font.bytes);
	const mappings = [...glyphs]
		.map(([glyph, code]) => `<${glyph.toString(16).padStart(4, "0")}> <${utf16(code)}>`)
		.join("\n");
	const cmap = [
		"/CIDInit /ProcSet findresource begin 12 dict begin begincmap",
		"/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def",
		"/CMapName /Adobe-Identity-UCS def /CMapType 2 def",
		"1 begincodespacerange <0000> <FFFF> endcodespacerange",
		`${glyphs.size} beginbfchar`,
		mappings,
		"endbfchar endcmap CMapName currentdict /CMap defineresource pop end end",
	].join("\n");
	return [
		`<< /Type /Font /Subtype /Type0 /BaseFont /${name} /Encoding /Identity-H /DescendantFonts [${base + 1} 0 R] /ToUnicode ${base + 4} 0 R >>`,
		`<< /Type /Font /Subtype /${font.cff ? "CIDFontType0" : "CIDFontType2"} /BaseFont /${name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor ${base + 2} 0 R /DW 1000 /W [${widths}]${font.cff ? "" : " /CIDToGIDMap /Identity"} >>`,
		`<< /Type /FontDescriptor /FontName /${name} /Flags 32 /FontBBox [${font.bbox.map((value) => Math.round(value * scale)).join(" ")}] /ItalicAngle 0 /Ascent ${Math.round(font.ascent * scale)} /Descent ${Math.round(font.descent * scale)} /CapHeight ${Math.round(font.ascent * scale)} /StemV 80 /${font.cff ? "FontFile3" : "FontFile2"} ${base + 3} 0 R >>`,
		`<< ${font.cff ? "/Subtype /OpenType" : `/Length1 ${font.bytes.length}`} /Filter /ASCIIHexDecode /Length ${program.length} >>\nstream\n${program}\nendstream`,
		`<< /Length ${cmap.length} >>\nstream\n${cmap}\nendstream`,
	];
}

function hex(bytes: Uint8Array): string {
	let out = "";
	for (let index = 0; index < bytes.length; index++) {
		out += bytes[index].toString(16).padStart(2, "0");
		if (index % 64 === 63) out += "\n";
	}
	return `${out}>`;
}

function utf16(code: number): string {
	const units =
		code > 0xffff ? [0xd800 + ((code - 0x10000) >> 10), 0xdc00 + ((code - 0x10000) & 0x3ff)] : [code];
	return units.map((unit) => unit.toString(16).padStart(4, "0")).join("");
}

/** The typefaces the text on these items is set in, and osifont whenever another one needs it. */
export function printFontIds(items: ReadonlyArray<{ kind: string; font?: string }>): string[] {
	const ids = new Set(
		items.flatMap((item) => (item.kind === "text" && item.font && fileOf(item.font) ? [item.font] : [])),
	);
	if (ids.size) ids.add(CAD_FALLBACK_FONT_ID);
	return [...ids];
}

function fileOf(id: string): string | null {
	return CAD_FONTS.find((font) => font.id === id)?.file ?? null;
}

/**
 * Reads the font files the text on these items needs, as the app serves them. A font that cannot
 * be read is left out, and its text prints in Helvetica rather than failing the export.
 */
export async function loadPrintFonts(
	items: ReadonlyArray<{ kind: string; font?: string }>,
	read: (file: string) => Promise<Uint8Array> = async (file) =>
		new Uint8Array(await (await fetch(`/fonts/cad/${file}`)).arrayBuffer()),
): Promise<PrintFontSet> {
	const loaded = await Promise.all(
		printFontIds(items).map(async (id) => {
			try {
				return [id, parseFont(await read(fileOf(id) ?? ""))] as const;
			} catch {
				return null;
			}
		}),
	);
	return new PrintFontSet(new Map(loaded.flatMap((entry) => (entry ? [entry] : []))));
}
