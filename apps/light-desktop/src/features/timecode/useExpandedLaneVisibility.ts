import { useLayoutEffect, type RefObject } from "react";

/** Reveal the whole edited lane vertically without changing its horizontal time origin. */
export function useExpandedLaneVisibility(
	viewportRef: RefObject<HTMLDivElement | null>,
	laneId: string | null,
	cueId: string | null,
) {
	useLayoutEffect(() => {
		if (!cueId || !laneId) return;
		const viewport = viewportRef.current;
		if (!viewport) return;
		const lane = Array.from(viewport.querySelectorAll<HTMLElement>(".timecode-editor-lane"))
			.find((candidate) => candidate.dataset.laneId === laneId);
		if (!lane) return;
		const reveal = () => {
			const bounds = viewport.getBoundingClientRect();
			const heading = viewport.querySelector<HTMLElement>(".timecode-timeline-heading");
			const audio = viewport.querySelector<HTMLElement>(".timecode-editor-lane.main-audio");
			const scrollportTop = bounds.top + viewport.clientTop;
			const bottom = scrollportTop + viewport.clientHeight;
			const top = scrollportTop + (heading?.offsetHeight ?? 0) + (audio?.offsetHeight ?? 0);
			const rect = lane.getBoundingClientRect();
			if (rect.bottom > bottom) viewport.scrollTop += rect.bottom - bottom;
			else if (rect.top < top) viewport.scrollTop -= top - rect.top;
		};
		reveal();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(reveal);
		observer.observe(viewport);
		observer.observe(lane);
		return () => observer.disconnect();
	}, [viewportRef, laneId, cueId]);
}
