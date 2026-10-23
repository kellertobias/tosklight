import { expect, it, vi } from "vitest";
import { overviewResizer } from "./timelineOverviewResize";
import type { OverviewResize } from "./timecodeEditorShared";
function setup() {
	const setZoom = vi.fn();
	const setOverviewResize = vi.fn();
	const resize = overviewResizer({ timelineViewportWidth: 1200, duration: 440, maximumZoom: 20, fitPixelsPerFrame: 1, setZoom, setOverviewResize });
	return { resize, setZoom, setOverviewResize };
}
it("refits the current viewport when both overview edges explicitly reach the track ends", () => {
	const { resize, setZoom, setOverviewResize } = setup();
	resize(0.0007, 0.9993);
	expect(setZoom).toHaveBeenCalledExactlyOnceWith(1);
	const state = setOverviewResize.mock.calls[0][0](null) as OverviewResize;
	expect(state).toMatchObject({ zoom: 1, startFraction: 0, revision: 1 });
});
it("keeps partial overview ranges on the existing fit reference", () => {
	const { resize, setZoom, setOverviewResize } = setup();
	resize(0.2, 0.8);
	expect(setZoom.mock.calls[0][0]).toBeCloseTo(1200 / 0.6 / 440);
	expect(setOverviewResize.mock.calls[0][0](null).startFraction).toBe(0.2);
});
