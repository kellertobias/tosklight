import { Button } from "@tosklight/ui";
import { useOverviewDrag } from "./useOverviewDrag";
import { type CSSProperties, type RefObject } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	displayedTimecodeLanes,
	markerColorOption,
	type TimelineItem,
} from "./timecodeEditorShared";

export function TimelineOverview({
	definition,
	items,
	pixelsPerFrame,
	scrollLeft,
	viewportWidth,
	totalWidth,
	scrollableWidth,
	scrollRef,
	viewportId,
	maximumPixelsPerFrame,
	onScrollLeftChange,
	onVisibleRangeChange,
}: {
	definition: TimecodeDefinition;
	items: readonly TimelineItem[];
	pixelsPerFrame: number;
	scrollLeft: number;
	viewportWidth: number;
	totalWidth: number;
	scrollableWidth: number;
	scrollRef: RefObject<HTMLDivElement | null>;
	viewportId: string;
	maximumPixelsPerFrame: number;
	onScrollLeftChange(scrollLeft: number): void;
	onVisibleRangeChange(startFraction: number, endFraction: number, displayedFrames: number): void;
}) {
	const {
		maximumScroll,
		visibleWidth,
		visibleLeft,
		beginDrag,
		moveDrag,
		endDrag,
		panTo,
	} = useOverviewDrag({
		scrollRef,
		totalWidth,
		scrollableWidth,
		displayedFrames: totalWidth / pixelsPerFrame,
		viewportWidth,
		scrollLeft,
		minimumVisibleFraction: Math.min(1, viewportWidth / ((totalWidth / pixelsPerFrame) * maximumPixelsPerFrame)),
		onScrollLeftChange,
		onVisibleRangeChange,
	});
	// Project the shared frame axis into retained overview context. Keeping this
	// context stable during a drag never adds blank native scroll reach.
	const framePercent = (frame: number) => (frame * pixelsPerFrame / totalWidth) * 100;
	return (
		<div
			className="timecode-timeline-overview"
			data-displayed-frames={totalWidth / pixelsPerFrame}
			role="scrollbar"
			aria-label="Timeline overview"
			aria-controls={viewportId}
			aria-orientation="horizontal"
			aria-valuemin={0}
			aria-valuemax={Math.round(maximumScroll)}
			aria-valuenow={Math.round(scrollLeft)}
			tabIndex={0}
			onPointerDown={(event) => {
				event.currentTarget.setPointerCapture(event.pointerId);
				beginDrag(event, event.currentTarget);
			}}
			onPointerMove={(event) => {
				if (event.currentTarget.hasPointerCapture(event.pointerId))
					moveDrag(event.clientX, event.currentTarget);
			}}
			onPointerUp={(event) => {
				endDrag(event.pointerId);
			}}
			onPointerCancel={(event) => {
				endDrag(event.pointerId);
			}}
			onKeyDown={(event) => {
				if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
				event.preventDefault();
				const direction = event.key === "ArrowRight" ? 1 : -1;
				if (scrollRef.current)
					panTo(
						(scrollRef.current.scrollLeft + direction * viewportWidth * 0.1) /
							totalWidth,
					);
			}}
		>
			<OverviewLanes definition={definition} items={items} framePercent={framePercent} />
			{definition.markers.map((marker) => (
				<i
					key={marker.id}
					className="timecode-timeline-overview-marker"
					data-marker-id={marker.id}
					style={{
						left: `${framePercent(marker.frame)}%`,
						background: markerColorOption(marker.color).value,
					}}
					aria-hidden="true"
				/>
			))}
			<span
				className="timecode-timeline-overview-visible"
				style={{ left: `${visibleLeft}%`, width: `${visibleWidth}%` }}
			>
				<Button size="compact" className="timecode-timeline-overview-move" data-overview-pan
					aria-label="Move visible timeline range" title="Drag to move the visible time range; arrow keys pan">
					<span aria-hidden="true"><svg viewBox="0 0 18 8"><circle cx="4" cy="2" r="1"/><circle cx="9" cy="2" r="1"/><circle cx="14" cy="2" r="1"/><circle cx="4" cy="6" r="1"/><circle cx="9" cy="6" r="1"/><circle cx="14" cy="6" r="1"/></svg></span>
				</Button>
				<i
					className="timecode-timeline-overview-handle handle-start"
					data-overview-edge="start"
					role="separator"
					aria-label="Resize timeline overview from start"
					aria-orientation="vertical"
				/>
				<i
					className="timecode-timeline-overview-handle handle-end"
					data-overview-edge="end"
					role="separator"
					aria-label="Resize timeline overview from end"
					aria-orientation="vertical"
				/>
			</span>
		</div>
	);
}

function OverviewLanes({ definition, items, framePercent }: {
	definition: TimecodeDefinition;
	items: readonly TimelineItem[];
	framePercent(frame: number): number;
}) {
	const { lanes, laneHeight, laneGap } = overviewLanes(definition);
	return (
			<div
				className="timecode-timeline-overview-lanes"
				style={
					{
						"--timecode-overview-lane-height": `${laneHeight}px`,
						"--timecode-overview-lane-gap": `${laneGap}px`,
					} as CSSProperties
				}
				data-lane-height={laneHeight}
				aria-hidden="true"
			>
				{lanes.map((lane) => (
					<span
						key={lane.id}
						className={`timecode-timeline-overview-lane overview-lane-${lane.kind}`}
						data-overview-lane-id={lane.id}
					>
						{items
							.filter((item) => item.laneId === lane.id)
							.map((item) => (
								<i
									key={item.selection.itemId}
									data-item-id={item.selection.itemId}
									style={{
										left: `${framePercent(item.frame)}%`,
										width: item.endFrame
											? `${framePercent(item.endFrame - item.frame)}%`
											: undefined,
									}}
								/>
							))}
					</span>
				))}
			</div>
	);
}

function overviewLanes(definition: TimecodeDefinition) {
	const lanes = [
		...(definition.audio ? [{ id: "main-audio", kind: "audio" }] : []),
		...displayedTimecodeLanes(definition).map((lane) => ({
			id: lane.id,
			kind: lane.content.kind,
		})),
	];
	const laneCount = Math.max(1, lanes.length);
	const laneHeight = Math.max(1, Math.min(3, Math.floor(44 / laneCount)));
	const laneGap = laneHeight * laneCount + laneCount - 1 <= 44 ? 1 : 0;
	return { lanes, laneHeight, laneGap };
}
