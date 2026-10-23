import { expect, it, vi } from "vitest";
import { overviewResizer } from "./timelineOverviewResize";
import type { OverviewResize } from "./timecodeEditorShared";
function setup(viewport = 1200) {
	const fitTimeline = vi.fn();
	const preserveOverviewExtent = vi.fn();
	const setZoom = vi.fn();
	const setOverviewResize = vi.fn();
	const resize = overviewResizer({ timelineViewportWidth: viewport, preserveOverviewExtent, maximumZoom: 20, fitPixelsPerFrame: 1, setZoom, setOverviewResize, fitTimeline });
	return { resize, setZoom, setOverviewResize, preserveOverviewExtent, fitTimeline };
}
it("refits the current viewport when both overview edges explicitly reach the track ends", () => {
	const { resize, setZoom, setOverviewResize, fitTimeline, preserveOverviewExtent } = setup();
	resize(0.0007, 0.9993, 440);
	expect(fitTimeline).toHaveBeenCalledOnce();
	expect(setZoom).not.toHaveBeenCalled();
	expect(preserveOverviewExtent).not.toHaveBeenCalled();
	const state = setOverviewResize.mock.calls[0][0](null) as OverviewResize;
	expect(state).toMatchObject({ zoom: 1, startFrame: 0, revision: 1 });
});
it("keeps partial overview ranges on the existing fit reference", () => {
	const { resize, setZoom, setOverviewResize } = setup();
	resize(0.2, 0.8, 440);
	expect(setZoom.mock.calls[0][0]).toBeCloseTo(1200 / 0.6 / 440);
	expect(setOverviewResize.mock.calls[0][0](null).startFrame).toBe(88);
});

it("zooms a displayed range including blank space and anchors it by its frame", () => {
	const { resize, setZoom, setOverviewResize, preserveOverviewExtent } = setup();
	// Only 440 frames contain the Timecode; the preserved canvas displays 1000.
	resize(0.25, 0.75, 1000);
	expect(setZoom).toHaveBeenCalledExactlyOnceWith(2.4);
	expect(preserveOverviewExtent).toHaveBeenCalledExactlyOnceWith(1000);
	expect(setOverviewResize.mock.calls[0][0](null).startFrame).toBe(250);
});

it("retains the canvas and start frame when a partial range reaches minimum zoom", () => {
	const { resize, setZoom, setOverviewResize, preserveOverviewExtent, fitTimeline } = setup(600);
	resize(0.1, 0.9, 1000);
	expect(setZoom).toHaveBeenCalledExactlyOnceWith(1);
	expect(preserveOverviewExtent).toHaveBeenCalledExactlyOnceWith(1000);
	expect(setOverviewResize.mock.calls[0][0](null).startFrame).toBe(100);
	expect(fitTimeline).not.toHaveBeenCalled();
});
