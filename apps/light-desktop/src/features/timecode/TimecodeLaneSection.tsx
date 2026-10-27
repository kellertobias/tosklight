import { Button } from "@tosklight/ui";
import type { ReactNode } from "react";
import type { TimecodeLane } from "../../api/types/timecode";
import { speedGroupLinePoints } from "./speedLaneGeometry";

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
	onToggle,
	renderLane,
}: {
	title: string;
	className: string;
	lanes: readonly TimecodeLane[];
	collapsed: boolean;
	duration: number;
	onToggle(): void;
	renderLane(lane: TimecodeLane): ReactNode;
}) {
	if (!lanes.length) return null;
	if (lanes[0].content.kind === "cue_list")
		return (
			<section
				className={`timecode-lane-section ${className}`}
				aria-label="Cuelist lanes"
			>
				{lanes.map(renderLane)}
			</section>
		);
	return (
		<section
			className={`timecode-lane-section ${className} ${!collapsed ? "section-expanded" : ""}`}
			data-expanded={!collapsed}
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
						<span className="timecode-section-title">
							<strong>{title}</strong>
							<span className="timecode-section-hint">
								{collapsed ? "Expand" : "Collapse"}
							</span>
						</span>
						<small>{lanes.length}</small>
					</Button>
				</div>
				{collapsed && (
					<Button
						className={`timecode-section-summary ${className === "timecode-speed-section" ? "timecode-speed-summary" : ""}`}
						aria-label={`Expand ${title}`}
						aria-description={lanes.map((lane) => lane.name).join(", ")}
						title={lanes.map((lane) => lane.name).join(" · ")}
						onClick={onToggle}
						onPointerDown={(event) => event.stopPropagation()}
					>
						<span className="timecode-section-miniatures">
							{lanes.map((lane) => (
								<span
									key={lane.id}
									className="timecode-section-miniature"
									data-lane-id={lane.id}
									data-lane-name={lane.name}
								>
									<Miniature lane={lane} duration={duration} />
								</span>
							))}
						</span>
						<span className="timecode-section-preview-hint">Expand</span>
					</Button>
				)}
			</div>
			{!collapsed && lanes.map(renderLane)}
		</section>
	);
}

function Miniature({
	lane,
	duration,
}: {
	lane: TimecodeLane;
	duration: number;
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
