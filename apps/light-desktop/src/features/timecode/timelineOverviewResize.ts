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
		const requestedFraction = Math.max(0.0001, endFraction - startFraction);
		const requestedPixelsPerFrame = Math.min(
			TARGET_MAX_PIXELS_PER_FRAME,
			timelineViewportWidth / requestedFraction / duration,
		);
		const nextZoom = Math.max(
			1,
			Math.min(maximumZoom, requestedPixelsPerFrame / fitPixelsPerFrame),
		);
		setOverviewResize((current) => ({
			zoom: nextZoom,
			startFraction,
			revision: (current?.revision ?? 0) + 1,
		}));
		setZoom(nextZoom);
	};
}
