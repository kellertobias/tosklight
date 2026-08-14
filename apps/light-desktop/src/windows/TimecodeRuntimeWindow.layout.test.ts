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

	it("keeps lane titles fixed before the timeline origin and puts zoom below the viewport", () => {
		expect(css).toMatch(
			/\.timecode-timeline-editor\s*\{[\s\S]*?--timecode-lane-header-width:\s*10rem;/,
		);
		expect(css).toMatch(
			/\.timecode-ruler\s*\{[\s\S]*?top:\s*0;[\s\S]*?left:\s*var\(--timecode-lane-header-width\);/,
		);
		expect(css).toMatch(
			/\.timecode-editor-lane-label\s*\{[\s\S]*?position:\s*sticky;[\s\S]*?left:\s*0;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-tools\s*\{[\s\S]*?border-top:\s*1px solid #343644;/,
		);
	});
});
