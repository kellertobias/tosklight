import { describe, expect, it } from "vitest";
import { surfaceFromLocation } from "./surface";

describe("the surface a window loads", () => {
	it("is the CAD editor only for the CAD window the desktop app opens", () => {
		expect(surfaceFromLocation("?surface=cad")).toBe("cad");
		expect(surfaceFromLocation("?surface=cad&debug=1")).toBe("cad");
		expect(surfaceFromLocation("")).toBe("editor");
		expect(surfaceFromLocation("?surface=")).toBe("editor");
		expect(surfaceFromLocation("?surface=editor")).toBe("editor");
		// A near miss stays the editor rather than opening a surface nothing asked for.
		expect(surfaceFromLocation("?surface=CAD")).toBe("editor");
	});
});
