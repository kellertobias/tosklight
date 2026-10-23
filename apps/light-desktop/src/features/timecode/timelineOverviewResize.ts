import { TARGET_MAX_PIXELS_PER_FRAME } from "./useTimelineGeometry";
import type { OverviewResize } from "./timecodeEditorShared";

export function overviewResizer({
	timelineViewportWidth,
	maximumZoom,
	fitPixelsPerFrame,
	setOverviewResize,
	setZoom,
	fitTimeline,
}: {
	timelineViewportWidth: number;
	maximumZoom: number;
	fitPixelsPerFrame: number;
	setOverviewResize: import("react").Dispatch<
		import("react").SetStateAction<OverviewResize | null>
	>;
	setZoom(value: number): void;
	fitTimeline(): void;
}) {
	return (startFraction: number, endFraction: number, displayedFrames: number) => {
		// Taking both overview edges to the track ends is an explicit Fit gesture.
		// Allow two screen pixels so a touch does not need to land outside the track.
		const edgeTolerance = 2 / Math.max(1, timelineViewportWidth);
		const fullRange = startFraction <= edgeTolerance && endFraction >= 1 - edgeTolerance;
		const requestedFraction = Math.max(0.0001, endFraction - startFraction);
		const requestedPixelsPerFrame = Math.min(
			TARGET_MAX_PIXELS_PER_FRAME,
			timelineViewportWidth / requestedFraction / displayedFrames,
		);
		const nextZoom = fullRange ? 1 : Math.max(
			1,
			Math.min(maximumZoom, requestedPixelsPerFrame / fitPixelsPerFrame),
		);
		setOverviewResize((current) => ({
			zoom: nextZoom,
			startFrame: fullRange ? 0 : startFraction * displayedFrames,
			revision: (current?.revision ?? 0) + 1,
		}));
		if (fullRange) fitTimeline();
		else setZoom(nextZoom);
	};
}
