import { describe, expect, it } from "vitest";
import manifest from "../../public/fonts/cad/fonts.json";
import css from "./cadFonts.css?raw";
import { CAD_FONTS, cadFontFamily } from "./cadFonts";

// The test runs under Node; the app's own types know only the browser.
const fs: { readFileSync(path: string): Uint8Array; existsSync(path: string): boolean } = await import(
	/* @vite-ignore */ String("node:fs")
);
// Vitest runs from the app's own folder.
const { cwd } = (globalThis as unknown as { process: { cwd(): string } }).process;
const shipped = (file: string) => `${cwd()}/public/fonts/cad/${file}`;
const sha256 = async (bytes: Uint8Array) =>
	[...new Uint8Array(await crypto.subtle.digest("SHA-256", new Uint8Array(bytes)))]
		.map((byte) => byte.toString(16).padStart(2, "0"))
		.join("");

describe("the CAD text typefaces", () => {
	it("ships every offered typeface unaltered, with its source, licence and licence texts beside it", async () => {
		expect(manifest.fonts.map((font) => font.id)).toEqual(CAD_FONTS.filter((font) => font.id).map((font) => font.id));
		for (const font of manifest.fonts) {
			expect(await sha256(fs.readFileSync(shipped(font.file))), font.file).toBe(font.sha256);
			expect(font.source).toMatch(/^https:\/\/github\.com\//u);
			expect(font.license).not.toBe("");
			for (const licence of font.licenseFiles) expect(fs.existsSync(shipped(licence)), licence).toBe(true);
			// The stylesheet loads the file the manifest names.
			expect(css).toContain(`/fonts/cad/${font.file}`);
		}
	});

	it("draws an unknown or absent typeface in the screen's own, and falls back to osifont per glyph", () => {
		expect(cadFontFamily(undefined)).toBeNull();
		expect(cadFontFamily("")).toBeNull();
		expect(cadFontFamily("a-typeface-from-a-later-release")).toBeNull();
		expect(cadFontFamily("osifont")).toContain("ToskLight CAD osifont");
		// Hershey has no umlauts or ß; those characters come from osifont, not the system.
		expect(cadFontFamily("hershey-simplex")).toMatch(/Hershey Simplex".*osifont/u);
	});
});
