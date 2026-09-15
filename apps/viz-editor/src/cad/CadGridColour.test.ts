import { describe, expect, it } from "vitest";
import { parseHexColour } from "./CadGridColour";

describe("a typed grid colour", () => {
	it("reads six or three hex digits, with or without the hash, in either case", () => {
		expect(parseHexColour("#C9D1D9")).toBe("#c9d1d9");
		expect(parseHexColour("c9d1d9")).toBe("#c9d1d9");
		expect(parseHexColour(" #abc ")).toBe("#aabbcc");
	});

	it("refuses anything that is not a colour", () => {
		for (const text of ["", "#abcd", "grey", "#ggg000", "rgb(1,2,3)"])
			expect(parseHexColour(text)).toBeNull();
	});
});
