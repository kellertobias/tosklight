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
		expect(css).toContain(".timecode-timeline-scroll::-webkit-scrollbar-thumb");
	});

	it("keeps lane titles fixed on the exact shared timeline origin", () => {
		expect(css).toMatch(
			/\.timecode-timeline-editor\s*\{[\s\S]*?--timecode-lane-header-width:\s*160px;[\s\S]*?border:\s*0;[\s\S]*?background:\s*transparent;/,
		);
		expect(css).toMatch(
			/\.timecode-ruler\s*\{[\s\S]*?top:\s*0;[\s\S]*?left:\s*var\(--timecode-lane-header-width\);/,
		);
		expect(css).toMatch(
			/\.timecode-editor-lane-label\s*\{[\s\S]*?position:\s*sticky;[\s\S]*?left:\s*0;/,
		);
		expect(css).toContain(".timecode-ruler .timecode-ruler-first-tick");
		expect(css).toMatch(
			/\.timecode-timeline-item\s*\{[\s\S]*?transform:\s*none;/,
		);
		expect(css).toMatch(
			/\.timecode-editor-lane\s*\{[\s\S]*?height:\s*var\(--timecode-lane-height\);[\s\S]*?min-height:\s*var\(--timecode-lane-height\);[\s\S]*?max-height:\s*var\(--timecode-lane-height\);/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-item\.item-clip\s*\{[\s\S]*?top:\s*3px;[\s\S]*?height:\s*calc\(100% - 6px\);[\s\S]*?min-height:\s*0;/,
		);
	});

	it("keeps the audio waveform inside a taller audio lane and fills its body", () => {
		expect(css).toMatch(
			/\.timecode-editor-lane\.lane-audio_volume\s*\{[\s\S]*?height:\s*calc\(var\(--timecode-lane-height\) \* 1\.5\);[\s\S]*?max-height:\s*calc\(var\(--timecode-lane-height\) \* 1\.5\);/,
		);
		expect(css).toMatch(
			/\.timecode-audio-lane-content \.timecode-waveform\s*\{[\s\S]*?position:\s*absolute;[\s\S]*?inset:\s*0;[\s\S]*?height:\s*100%;/,
		);
		expect(css).toMatch(
			/\.timecode-audio-lane-content \.timecode-waveform-envelope\s*\{[\s\S]*?fill:\s*#7a63b8;[\s\S]*?stroke:\s*none;/,
		);
	});

	it("splits the overview window between panning above and resizing below", () => {
		// However far the timeline is zoomed in, the window keeps a surface of each kind: the
		// upper half always pans it and the lower half always resizes its start and end.
		expect(css).toMatch(
			/\.timecode-timeline-overview-handle\s*\{[\s\S]*?top:\s*50%;[\s\S]*?bottom:\s*-1px;[\s\S]*?width:\s*min\(16px, 50%\);/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-overview-handle\.handle-start\s*\{[\s\S]*?left:\s*-1px;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-overview-handle\.handle-end\s*\{[\s\S]*?right:\s*-1px;/,
		);
	});

	it("does not cap the rendered width of long Timecode clips", () => {
		expect(css).toMatch(
			/\.timecode-timeline-item\.item-clip\s*\{[\s\S]*?max-width:\s*none;/,
		);
	});

	it("keeps marker touch width invisible and shows selection on the flag", () => {
		expect(css).toMatch(
			/\.ui-button\.timecode-timeline-marker\s*\{[\s\S]*?width:\s*44px;[\s\S]*?transform:\s*none;[\s\S]*?border:\s*0;[\s\S]*?background:\s*transparent;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-marker-line\s*\{[\s\S]*?left:\s*0;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-marker-label\s*\{[\s\S]*?bottom:\s*0\.25rem;[\s\S]*?left:\s*0;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-marker\.selected \.timecode-timeline-marker-line\s*\{[\s\S]*?box-shadow:\s*none;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-marker\.selected \.timecode-timeline-marker-label\s*\{[\s\S]*?background:\s*var\(--timecode-marker-color\);[\s\S]*?color:\s*var\(--timecode-marker-text-color, #fff\);/,
		);
	});

	it("draws a full-width fixed-height draggable timeline overview", () => {
		expect(css).toMatch(
			/\.timecode-timeline-overview\s*\{[\s\S]*?width:\s*100%;[\s\S]*?height:\s*54px;[\s\S]*?min-height:\s*54px;[\s\S]*?max-height:\s*54px;[\s\S]*?touch-action:\s*none;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-overview-lane\s*\{[\s\S]*?height:\s*var\(--timecode-overview-lane-height, 3px\);/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-overview-visible\s*\{[\s\S]*?border:\s*1px solid #58d4ef;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-overview-handle\s*\{[\s\S]*?width:\s*min\(16px, 50%\);[\s\S]*?cursor:\s*ew-resize;/,
		);
	});

	it("keeps the Cue List chooser pool scrollable with left-aligned titles", () => {
		expect(css).toMatch(
			/\.timecode-cuelist-chooser-scroll\s*\{[\s\S]*?min-height:\s*12rem;[\s\S]*?overflow:\s*auto;/,
		);
		expect(css).toMatch(
			/\.timecode-cuelist-chooser-grid \.pool-card-name\s*\{[\s\S]*?text-align:\s*left;/,
		);
		expect(css).toMatch(
			/\.timecode-lane-select\.ui-button\s*\{[\s\S]*?justify-content:\s*start;[\s\S]*?padding:\s*0\.35rem 0\.5rem;[\s\S]*?text-align:\s*left;/,
		);
		expect(css).toMatch(
			/\.timecode-editor-lane\.lane-cue_list \.timecode-lane-select\.ui-button\s*\{[\s\S]*?justify-items:\s*start;[\s\S]*?text-align:\s*left;/,
		);
	});

	it("draws the Speed Group graph without adding black footer chrome", () => {
		expect(css).toMatch(
			/\.timecode-speed-keyframe-curve\s*\{[\s\S]*?inset:\s*0 0 0 var\(--timecode-lane-header-width\);[\s\S]*?pointer-events:\s*none;/,
		);
		expect(css).toMatch(
			/\.timecode-keyframe-actions\s*\{[\s\S]*?background:\s*transparent;/,
		);
		expect(css).toMatch(
			/\.timecode-keyframe-actions-title\s*\{[\s\S]*?flex:\s*0 0 var\(--timecode-lane-header-width\);[\s\S]*?background:\s*transparent;[\s\S]*?text-align:\s*left;/,
		);
		expect(css).not.toContain(".timecode-keyframe-actions-title.selected");
		expect(css).toContain(".timecode-keyframe-value-control");
	});

	it("keeps the playhead label at its top and has only the exact ruler grid", () => {
		expect(css).toMatch(
			/\.ui-button\.timecode-editor-playhead\s*\{[\s\S]*?z-index:\s*3;/,
		);
		expect(css).toMatch(
			/\.timecode-editor-playhead span\s*\{[\s\S]*?position:\s*absolute;[\s\S]*?top:\s*0;/,
		);
		expect(css).toMatch(
			/\.timecode-timeline-canvas\s*\{[\s\S]*?background:\s*#11121a;/,
		);
		expect(css).not.toContain("repeating-linear-gradient");
	});
});
