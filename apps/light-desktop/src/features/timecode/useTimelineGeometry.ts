import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { TIMECODE_LANE_HEADER_WIDTH } from "./timecodeEditorShared";
import { timelineTrailingGutter } from "./timelineTrailingGutter";
export const TARGET_MAX_PIXELS_PER_FRAME = 17.5;
const FALLBACK_VIEWPORT_WIDTH = 720;
function useTimelineViewportWidth(scrollRef: RefObject<HTMLDivElement | null>) {
	const [viewportWidth, setViewportWidth] = useState(FALLBACK_VIEWPORT_WIDTH);
	useLayoutEffect(() => {
		const element = scrollRef.current;
		if (!element) return;
		const measure = () => {
			setViewportWidth(
				element.clientWidth > 0 ? element.clientWidth : FALLBACK_VIEWPORT_WIDTH,
			);
		};
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(element);
		return () => observer.disconnect();
	}, [scrollRef]);
	return viewportWidth;
}

export function timelineZoomGeometry(
	durationFrames: number,
	viewportWidth: number,
) {
	const duration = Math.max(1, durationFrames);
	const viewport = Math.max(1, viewportWidth);
	const fit = Math.min(TARGET_MAX_PIXELS_PER_FRAME, viewport / duration);
	return {
		fitPixelsPerFrame: fit,
		maximumZoom: Math.max(
			1,
			Math.ceil((TARGET_MAX_PIXELS_PER_FRAME / fit) * 4) / 4,
		),
	};
}

export function useTimelineGeometry({
	scrollRef,
	duration,
	definitionId,
	scrollTimelineTo,
}: {
	scrollRef: RefObject<HTMLDivElement | null>;
	duration: number;
	definitionId: string;
	scrollTimelineTo(value: number): void;
}) {
	const [zoom, setZoomValue] = useState(1);
	const [trailingExtent, setTrailingExtent] = useState(0);
	const restoreLeft = useRef<number | null>(null);
	const [fitReference, setFitReference] = useState<{
		id: string;
		scale: number;
	} | null>(null);
	const viewportWidth = useTimelineViewportWidth(scrollRef);
	const timelineViewportWidth = Math.max(
		1,
		viewportWidth - TIMECODE_LANE_HEADER_WIDTH,
	);
	const measuredFit = timelineZoomGeometry(
		duration,
		timelineViewportWidth,
	).fitPixelsPerFrame;
	const fitPixelsPerFrame =
		fitReference?.id === definitionId ? fitReference.scale : measuredFit;
	const maximumZoom = Math.max(
		1,
		Math.ceil((TARGET_MAX_PIXELS_PER_FRAME / fitPixelsPerFrame) * 4) / 4,
	);
	useLayoutEffect(() => {
		if (fitReference?.id === definitionId || !scrollRef.current?.clientWidth)
			return;
		setFitReference({ id: definitionId, scale: measuredFit });
	}, [definitionId, fitReference, measuredFit]);
	const prepareInspectorClose = () => {
		const viewport = scrollRef.current;
		if (!viewport) return;
		const body = viewport.closest<HTMLElement>(".timecode-editor-body");
		const expandedWidth = Math.max(
			viewport.clientWidth,
			body?.clientWidth ?? viewport.clientWidth,
		);
		const left = viewport.scrollLeft;
		// Reserve before React removes the inspector. Waiting for ResizeObserver is too
		// late: the browser has already clamped scrollLeft and emitted a scroll event.
		setTrailingExtent(
			Math.max(0, left + expandedWidth - TIMECODE_LANE_HEADER_WIDTH),
		);
		restoreLeft.current = left;
	};
	useLayoutEffect(() => {
		if (restoreLeft.current === null) return;
		const left = restoreLeft.current;
		restoreLeft.current = null;
		scrollTimelineTo(left);
	});
	const fitTimeline = () => {
		setTrailingExtent(0);
		restoreLeft.current = null;
		setFitReference({ id: definitionId, scale: measuredFit });
		setZoomValue(1);
		scrollTimelineTo(0);
	};
	const setZoom = (value: number) => {
		if (value <= 1) fitTimeline();
		else setZoomValue(Math.min(maximumZoom, value));
	};
	const pixelsPerFrame = Math.min(
		TARGET_MAX_PIXELS_PER_FRAME,
		fitPixelsPerFrame * zoom,
	);
	// Preserve the leftmost frame even when closing an inspector at the right edge.
	// The extra trailing extent is viewport space, never a change in the time scale.
	const width = Math.max(
		timelineViewportWidth,
		duration * pixelsPerFrame,
		trailingExtent,
	);
	useLayoutEffect(() => {
		const viewport = scrollRef.current;
		if (!viewport) return;
		const style = getComputedStyle(viewport);
		const borders = (Number.parseFloat(style.borderLeftWidth) || 0) +
			(Number.parseFloat(style.borderRightWidth) || 0);
		const gutter = timelineTrailingGutter(
			width + TIMECODE_LANE_HEADER_WIDTH, viewport.clientWidth,
			viewport.offsetWidth, borders,
		);
		viewport.style.setProperty("--timecode-trailing-scroll-gutter", `${gutter}px`);
	}, [scrollRef, width, viewportWidth]);
	return {
		viewportWidth,
		timelineViewportWidth,
		maximumZoom,
		fitPixelsPerFrame,
		pixelsPerFrame,
		width,
		zoom,
		setZoom,
		fitTimeline,
		prepareInspectorClose,
	};
}
