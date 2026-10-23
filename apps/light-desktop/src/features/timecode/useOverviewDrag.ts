import {
	useRef,
	type RefObject,
	type PointerEvent as ReactPointerEvent,
} from "react";
interface OverviewDragOptions {
	scrollRef: RefObject<HTMLDivElement | null>;
	totalWidth: number;
	displayedFrames: number;
	viewportWidth: number;
	scrollLeft: number;
	minimumVisibleFraction: number;
	onScrollLeftChange(left: number): void;
	onVisibleRangeChange(start: number, end: number, displayedFrames: number): void;
}
/** Panning and edge-resizing share one captured overview gesture. */
export function useOverviewDrag({
	scrollRef,
	totalWidth,
	displayedFrames,
	viewportWidth,
	scrollLeft,
	minimumVisibleFraction,
	onScrollLeftChange,
	onVisibleRangeChange,
}: OverviewDragOptions) {
	const maximumScroll = Math.max(0, totalWidth - viewportWidth);
	const visibleWidth = Math.min(100, (viewportWidth / totalWidth) * 100);
	const visibleLeft = Math.min(
		100 - visibleWidth,
		Math.max(0, (scrollLeft / totalWidth) * 100),
	);
	const visibleStart = visibleLeft / 100;
	const visibleEnd = (visibleLeft + visibleWidth) / 100;
	const drag = useRef<{
		pointerId: number;
		mode: "start" | "end" | "pan";
		grabOffset: number;
		displayedFrames: number;
		visibleStart: number;
		visibleEnd: number;
		minimumVisibleFraction: number;
	} | null>(null);
	const fractionAt = (clientX: number, overview: HTMLElement) => {
		const bounds = overview.getBoundingClientRect();
		return Math.max(
			0,
			Math.min(1, (clientX - bounds.left) / Math.max(1, bounds.width)),
		);
	};
	const panTo = (leftFraction: number) => {
		const next = Math.max(
			0,
			Math.min(maximumScroll, leftFraction * totalWidth),
		);
		if (scrollRef.current) scrollRef.current.scrollLeft = next;
		onScrollLeftChange(next);
	};
	const beginDrag = (
		event: ReactPointerEvent<HTMLDivElement>,
		overview: HTMLElement,
	) => {
		const fraction = fractionAt(event.clientX, overview);
		const edge =
			event.target instanceof Element
				? event.target.closest<HTMLElement>("[data-overview-edge]")?.dataset
						.overviewEdge
				: undefined;
		if (edge === "start" || edge === "end") {
			drag.current = {
				pointerId: event.pointerId,
				mode: edge,
				grabOffset: 0,
				displayedFrames, visibleStart, visibleEnd, minimumVisibleFraction,
			};
			return;
		}
		const inside = fraction >= visibleStart && fraction <= visibleEnd;
		const grabOffset = inside ? fraction - visibleStart : visibleWidth / 200;
		drag.current = {
			pointerId: event.pointerId,
			mode: "pan",
			grabOffset,
			displayedFrames, visibleStart, visibleEnd, minimumVisibleFraction,
		};
		panTo(fraction - grabOffset);
	};
	const moveDrag = (clientX: number, overview: HTMLElement) => {
		const current = drag.current;
		if (!current) return;
		const fraction = fractionAt(clientX, overview);
		if (current.mode === "start") {
			onVisibleRangeChange(
				Math.max(0, Math.min(current.visibleEnd - current.minimumVisibleFraction, fraction)),
				current.visibleEnd,
				current.displayedFrames,
			);
			return;
		}
		if (current.mode === "end") {
			onVisibleRangeChange(
				current.visibleStart,
				Math.min(1, Math.max(current.visibleStart + current.minimumVisibleFraction, fraction)),
				current.displayedFrames,
			);
			return;
		}
		panTo(fraction - current.grabOffset);
	};

	const endDrag = (pointerId: number) => {
		if (drag.current?.pointerId === pointerId) drag.current = null;
	};
	return {
		maximumScroll,
		visibleWidth,
		visibleLeft,
		beginDrag,
		moveDrag,
		endDrag,
		panTo,
	};
}
