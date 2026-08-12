import { describe, expect, it } from "vitest";
import css from "./TimecodeRuntimeWindow.css?raw";

describe("Timecode timeline layout", () => {
	it("fills the remaining pane and keeps both overflow axes visibly scrollable", () => {
		expect(css).toMatch(
			/\.timecode-timeline-editor\s*\{[\s\S]*?flex:\s*1 1 auto;[\s\S]*?min-height:\s*0;[\s\S]*?margin:\s*0;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-scroll\s*\{[\s\S]*?min-height:\s*0;[\s\S]*?overflow:\s*auto;[\s\S]*?scrollbar-width:\s*auto;[\s\S]*?scrollbar-gutter:\s*stable;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-canvas\s*\{[\s\S]*?min-height:\s*max\(20rem, 100%\);/,
		);
		expect(css).toContain(
			".timecode-timeline-scroll::-webkit-scrollbar-thumb",
		);
	});
});
