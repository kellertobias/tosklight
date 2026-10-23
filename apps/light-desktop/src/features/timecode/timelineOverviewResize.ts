import { TARGET_MAX_PIXELS_PER_FRAME } from "./useTimelineGeometry";
import type { OverviewResize } from "./timecodeEditorShared";

export function overviewResizer({
	timelineViewportWidth,
	duration,
	maximumZoom,
	fitPixelsPerFrame,
	setOverviewResize,
	setZoom,
}: {
	timelineViewportWidth: number;
	duration: number;
	maximumZoom: number;
	fitPixelsPerFrame: number;
	setOverviewResize: import("react").Dispatch<
		import("react").SetStateAction<OverviewResize | null>
	>;
	setZoom(value: number): void;
}) {
	return (startFraction: number, endFraction: number) => {
		// Taking both overview edges to the track ends is an explicit Fit gesture.
		// Allow two screen pixels so a touch does not need to land outside the track.
		const edgeTolerance = 2 / Math.max(1, timelineViewportWidth);
		const fullRange = startFraction <= edgeTolerance && endFraction >= 1 - edgeTolerance;
		const requestedFraction = Math.max(0.0001, endFraction - startFraction);
		const requestedPixelsPerFrame = Math.min(
			TARGET_MAX_PIXELS_PER_FRAME,
			timelineViewportWidth / requestedFraction / duration,
		);
		const nextZoom = fullRange ? 1 : Math.max(
			1,
			Math.min(maximumZoom, requestedPixelsPerFrame / fitPixelsPerFrame),
		);
		setOverviewResize((current) => ({
			zoom: nextZoom,
			startFraction: fullRange ? 0 : startFraction,
			revision: (current?.revision ?? 0) + 1,
		}));
		setZoom(nextZoom);
	};
}
