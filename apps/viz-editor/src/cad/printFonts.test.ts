import { describe, expect, it } from "vitest";
import type { CadAnnotation } from "./annotations";
import { buildCadPdf } from "./print";
import { loadPrintFonts, parseFont, printFontIds } from "./printFonts";
import type { CadPrintPage, CadSceneSnapshot } from "./types";

// The test runs under Node; the app's own types know only the browser.
const fs: {
	readFileSync(path: string): Uint8Array;
	writeFileSync(path: string, data: Uint8Array): void;
} = await import(/* @vite-ignore */ String("node:fs"));
const { process } = globalThis as unknown as { process: { cwd(): string; env: Record<string, string | undefined> } };
const read = async (file: string) => new Uint8Array(fs.readFileSync(`${process.cwd()}/public/fonts/cad/${file}`));

const scene: CadSceneSnapshot = {
	showId: "show",
	sceneRevision: 1,
	selectionRevision: 1,
	selectedIds: [],
	attachments: [],
	drawings: [],
	entities: [],
};
const page: CadPrintPage = {
	id: "plan",
	tileId: "tile",
	name: "Plan",
	view: "top_down",
	rotationQuarterTurns: 0,
	centreMillimetres: [0, 0],
	widthMillimetres: 5000,
	included: true,
	orientation: "landscape",
	showFixtureIds: true,
	showDmxAddresses: true,
	showMountingHardware: true,
};
const words = (id: string, text: string, y: number, font?: string): CadAnnotation => ({
	id,
	view: "top_down",
	kind: "text",
	points: [[-2000, y]],
	closed: false,
	text,
	textHeightMillimetres: 200,
	...(font ? { font } : {}),
});
const annotations = [
	words("screen", "FOH Screen", 1200),
	words("iso", "FOH Bühne ISO", 600, "osifont"),
	words("simplex", "FOH Bühne Größe", 0, "hershey-simplex"),
	words("complex", "Truss Größe", -600, "hershey-complex"),
];
const latin1 = (bytes: Uint8Array) => Array.from(bytes, (byte) => String.fromCharCode(byte)).join("");

describe("CAD typefaces in a printed plan", () => {
	it("reads the glyphs, advances and outlines kind of every shipped font", async () => {
		const osifont = parseFont(await read("osifont-lgpl3fe.ttf"));
		expect(osifont.cff).toBe(false);
		expect(osifont.postscriptName).toBe("osifont");
		expect(osifont.glyph("ü".codePointAt(0) ?? 0)).toBeGreaterThan(0);
		const simplex = parseFont(await read("AVHersheySimplexMedium.otf"));
		expect(simplex.cff).toBe(true);
		expect(simplex.glyph("A".codePointAt(0) ?? 0)).toBeGreaterThan(0);
		// Hershey has no umlauts: those come from osifont.
		expect(simplex.glyph("ü".codePointAt(0) ?? 0)).toBe(0);
		expect(simplex.advance(simplex.glyph(0x41))).toBeGreaterThan(0);
	});

	it("embeds the typefaces the text is set in, and osifont for what Hershey lacks", async () => {
		expect(printFontIds(annotations)).toEqual(["osifont", "hershey-simplex", "hershey-complex"]);
		const fonts = await loadPrintFonts(annotations, read);
		const pdf = buildCadPdf(scene, [page], undefined, [], annotations, fonts);
		const text = latin1(pdf);
		if (process.env.TOSKLIGHT_PRINT_FONTS_PDF) fs.writeFileSync(process.env.TOSKLIGHT_PRINT_FONTS_PDF, pdf);
		expect(text.startsWith("%PDF-1.6")).toBe(true);
		// osifont as a TrueType program, each Hershey face as an OpenType one, all composite.
		expect(text.match(/\/Subtype \/Type0/g)).toHaveLength(3);
		expect(text.match(/\/FontFile2/g)).toHaveLength(1);
		expect(text.match(/\/Subtype \/OpenType/g)).toHaveLength(2);
		expect(text).toContain("/ToUnicode");
		// The Hershey words switch to osifont for ü and back.
		const simplexLine = text.split("\n").find((line) => line.startsWith("BT") && line.includes("/CAD-hershey-simplex"));
		expect(simplexLine).toMatch(/CAD-hershey-simplex [\d.]+ Tf <[0-9a-f]+> Tj \/CAD-osifont [\d.]+ Tf <[0-9a-f]+> Tj \/CAD-hershey-simplex/u);
		// Text in the screen's own typeface stays in Helvetica.
		expect(text).toMatch(/\/F1 [\d.]+ Tf [\d. ]+Td \(FOH Screen\) Tj/u);
	});

	it("prints everything in Helvetica, as before, when no typeface could be read", async () => {
		const fonts = await loadPrintFonts(annotations, async () => {
			throw new Error("offline");
		});
		const text = latin1(buildCadPdf(scene, [page], undefined, [], annotations, fonts));
		expect(text.startsWith("%PDF-1.4")).toBe(true);
		expect(text).not.toContain("/Type0");
		expect(text).toMatch(/\/F1 [\d.]+ Tf [\d. ]+Td \(FOH B.+ ISO\) Tj/u);
	});
});

describe("Helvetica text in a printed plan", () => {
	it("prints umlauts, ß and WinAnsi punctuation as single escaped bytes, and the rest as a dash", async () => {
		const { winAnsiLiteral } = await import("./printPdfOps");
		expect(winAnsiLiteral("Bühne Größe")).toBe("B\\374hne Gr\\366\\337e");
		expect(winAnsiLiteral("12° ±5 €“x” (a\\b)")).toBe("12\\260 \\2615 \\200\\223x\\224 \\(a\\\\b\\)");
		expect(winAnsiLiteral("舞台")).toBe("--");
	});

	it("declares WinAnsi for Helvetica and keeps the page stream ASCII", async () => {
		const pdf = buildCadPdf(scene, [page], undefined, [], [words("screen", "Bühne Größe", 0)]);
		const text = latin1(pdf);
		if (process.env.TOSKLIGHT_PRINT_HELVETICA_PDF) fs.writeFileSync(process.env.TOSKLIGHT_PRINT_HELVETICA_PDF, pdf);
		expect(text).toContain("/BaseFont /Helvetica /Encoding /WinAnsiEncoding");
		expect(text).toContain("/BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding");
		expect(text).toContain("(B\\374hne Gr\\366\\337e) Tj");
		expect([...pdf].every((byte) => byte < 0x80)).toBe(true);
	});
});
