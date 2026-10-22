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
	duration,
	scrollLeft,
	viewportWidth,
	totalWidth,
	scrollRef,
	viewportId,
	minimumVisibleFraction,
	onScrollLeftChange,
	onVisibleRangeChange,
}: {
	definition: TimecodeDefinition;
	items: readonly TimelineItem[];
	duration: number;
	scrollLeft: number;
	viewportWidth: number;
	totalWidth: number;
	scrollRef: RefObject<HTMLDivElement | null>;
	viewportId: string;
	minimumVisibleFraction: number;
	onScrollLeftChange(scrollLeft: number): void;
	onVisibleRangeChange(startFraction: number, endFraction: number): void;
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
		viewportWidth,
		scrollLeft,
		minimumVisibleFraction,
		onScrollLeftChange,
		onVisibleRangeChange,
	});
	const { lanes, laneHeight, laneGap } = overviewLanes(definition);
	return (
		<div
			className="timecode-timeline-overview"
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
					>
						{items
							.filter((item) => item.laneId === lane.id)
							.map((item) => (
								<i
									key={item.selection.itemId}
									style={{
										left: `${(item.frame / duration) * 100}%`,
										width: item.endFrame
											? `${Math.max(0.25, ((item.endFrame - item.frame) / duration) * 100)}%`
											: undefined,
									}}
								/>
							))}
					</span>
				))}
			</div>
			{definition.markers.map((marker) => (
				<i
					key={marker.id}
					className="timecode-timeline-overview-marker"
					style={{
						left: `${(marker.frame / duration) * 100}%`,
						background: markerColorOption(marker.color).value,
					}}
					aria-hidden="true"
				/>
			))}
			<span
				className="timecode-timeline-overview-visible"
				style={{ left: `${visibleLeft}%`, width: `${visibleWidth}%` }}
			>
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
