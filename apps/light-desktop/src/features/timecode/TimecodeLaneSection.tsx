import { envelopePoints } from "./cueEnvelopeGeometry";
import { Button } from "@tosklight/ui";
import type { ReactNode } from "react";
import type { TimecodeLane } from "../../api/types/timecode";
import { cueClipTimingRows, type CueClipTimingDefaults } from "./cueClipTiming";
import { speedGroupLinePoints } from "./speedLaneGeometry";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

export const TIMECODE_LANE_SECTIONS = [
	{
		kind: "speed_group",
		title: "Speed Groups",
		className: "timecode-speed-section",
	},
	{
		kind: "cue_list",
		title: "Cuelists",
		className: "timecode-cuelist-section",
	},
	{
		kind: "audio_player",
		title: "Audio Players",
		className: "timecode-player-section",
	},
] as const;

/** Folding a section changes only the view; every preview uses the full timeline scale. */
export function TimecodeLaneSection({
	title,
	className,
	lanes,
	collapsed,
	duration,
	cueLists,
	timingDefaults,
	onToggle,
	onSelect,
	renderLane,
}: {
	title: string;
	className: string;
	lanes: readonly TimecodeLane[];
	collapsed: boolean;
	duration: number;
	cueLists: readonly TimecodeCueListOption[];
	timingDefaults: CueClipTimingDefaults;
	onToggle(): void;
	onSelect(laneId: string): void;
	renderLane(lane: TimecodeLane): ReactNode;
}) {
	if (!lanes.length) return null;
	return (
		<section
			className={`timecode-lane-section ${className}`}
			aria-label={`${title} lanes`}
		>
			<div className={`timecode-section-row ${collapsed ? "collapsed" : ""}`}>
				<div className="timecode-editor-lane-label">
					<Button
						className="timecode-section-toggle"
						aria-label={title}
						aria-expanded={!collapsed}
						onPointerDown={(event) => event.stopPropagation()}
						onClick={onToggle}
					>
						<span aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
						<strong>{title}</strong>
						<small>{lanes.length}</small>
					</Button>
				</div>
				{collapsed && (
					<div
						className={`timecode-section-summary ${className === "timecode-speed-section" ? "timecode-speed-summary" : ""}`}
					>
						{lanes.map((lane) => (
							<Button
								key={lane.id}
								className="timecode-section-miniature"
								data-lane-id={lane.id}
								aria-label={`Expand and edit ${lane.name}`}
								onClick={() => onSelect(lane.id)}
								onPointerDown={(event) => event.stopPropagation()}
							>
								<Miniature
									lane={lane}
									duration={duration}
									cueLists={cueLists}
									timingDefaults={timingDefaults}
								/>
								<span>{lane.name}</span>
							</Button>
						))}
					</div>
				)}
			</div>
			{!collapsed && lanes.map(renderLane)}
		</section>
	);
}

function Miniature({
	lane,
	duration,
	cueLists,
	timingDefaults,
}: {
	lane: TimecodeLane;
	duration: number;
	cueLists: readonly TimecodeCueListOption[];
	timingDefaults: CueClipTimingDefaults;
}) {
	const x = (frame: number) =>
		Math.max(0, Math.min(100, (frame / Math.max(1, duration)) * 100));
	const content = lane.content;
	return (
		<svg
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			aria-hidden="true"
			className={`miniature-${content.kind}`}
		>
			{content.kind === "speed_group" ? (
				<polyline points={speedGroupLinePoints(content.keyframes, duration)} />
			) : content.kind === "cue_list" ? (
				content.clips.flatMap((clip) => {
					const body = cueLists.find(
						(list) => list.id === content.cue_list_id,
					)?.body;
					if (!body)
						return (
							<rect
								key={clip.id}
								x={x(clip.start_frame)}
								y="20"
								width={x(clip.end_frame) - x(clip.start_frame)}
								height="60"
							/>
						);
					return cueClipTimingRows(clip, body, timingDefaults).rows.map(
						(row, index, rows) => {
							const last = index === rows.length - 1;
							return (
								<polygon
									key={`${clip.id}-${row.cue.id}`}
									points={envelopePoints(
										row.startFrame,
										row.inFade.startFrame,
										row.inFade.endFrame,
										last
											? Math.max(clip.end_frame, row.inFade.endFrame)
											: row.outFade.startFrame,
										last
											? Math.max(clip.end_frame, row.inFade.endFrame)
											: row.outFade.endFrame,
										90,
										15,
										clip.start_frame,
										clip.end_frame,
									)
										.split(" ")
										.map((point) => {
											const [frame, y] = point.split(",").map(Number);
											return `${x(frame)},${y}`;
										})
										.join(" ")}
								/>
							);
						},
					);
				})
			) : content.kind === "audio_player" ? (
				content.clips.map((clip) => (
					<rect
						key={clip.id}
						x={x(clip.start_frame)}
						y="20"
						width={x(clip.end_frame) - x(clip.start_frame)}
						height="60"
					/>
				))
			) : null}
		</svg>
	);
}
