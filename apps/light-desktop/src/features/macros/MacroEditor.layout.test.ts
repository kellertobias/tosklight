import { describe, expect, it } from "vitest";
import macroCss from "../../windows/MacrosWindow.css?raw";

describe("Macro Editor help layout", () => {
	it("keeps the source and help side by side, then presents help over the source at narrow pane widths", () => {
		expect(macroCss).toContain(
			"grid-template-columns: minmax(18rem, 1fr) clamp(16rem, 28cqw, 22rem);",
		);
		expect(macroCss).toContain("@container macro-editor (max-width: 44rem)");
		expect(macroCss).toContain("grid-template-rows: minmax(0, 1fr);");
		expect(macroCss).toContain("visibility: hidden;");
	});
});
