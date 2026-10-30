import { Button } from "@tosklight/ui";
import { createContext, useContext, useRef } from "react";
import { timelineFrameX } from "./timecodeEditorShared";
import "./timecodeBeatGrid.css";

export interface TimecodeSuggestedSection {
	frame: number;
	label: string;
}

export interface TimecodeBeatGrid {
	beatFrames: readonly number[];
	downbeatFrames: readonly number[];
	suggestions: readonly TimecodeSuggestedSection[];
	/** Whether dragged clips and items also snap to beats. */
	snap: boolean;
	/** Whether beat and bar lines are drawn; suggestions are drawn either way. */
	showBeats: boolean;
}

const BeatGridContext = createContext<TimecodeBeatGrid | undefined>(undefined);

/** Supplies the song analysis to the timeline inside it: drawing and beat snapping. */
export const TimecodeBeatGridProvider = BeatGridContext.Provider;

export function useTimecodeBeatGrid() {
	return useContext(BeatGridContext);
}

/** The beat frames a drag snaps to while beat snapping is on, read fresh by pointer handlers. */
export function useBeatSnapFrames() {
	const grid = useTimecodeBeatGrid();
	const frames = useRef<readonly number[] | undefined>(undefined);
	frames.current = grid?.snap ? grid.beatFrames : undefined;
	return frames;
}

/**
 * The song analysis drawn under the lanes: faint beat lines, stronger bar lines, and dashed
 * suggested section lines with their names, so suggestions never look like stored markers.
 */
export function TimecodeBeatGridLayer({ pixelsPerFrame }: { pixelsPerFrame: number }) {
	const grid = useTimecodeBeatGrid();
	if (!grid) return null;
	const bars = new Set(grid.downbeatFrames);
	// Beat lines closer than four pixels only add noise; bars stay until they are that close too.
	const beatsVisible = grid.showBeats && pixelsPerFrame * medianGap(grid.beatFrames) >= 4;
	const barsVisible = grid.showBeats && pixelsPerFrame * medianGap(grid.downbeatFrames) >= 4;
	return (
		<div className="timecode-beat-grid" aria-hidden="true">
			{grid.beatFrames.map((frame) =>
				bars.has(frame) ? (
					barsVisible && (
						<i key={frame} className="timecode-bar-line" style={{ left: timelineFrameX(frame, pixelsPerFrame) }} />
					)
				) : (
					beatsVisible && (
						<i key={frame} className="timecode-beat-line" style={{ left: timelineFrameX(frame, pixelsPerFrame) }} />
					)
				),
			)}
			{grid.suggestions.map((section) => (
				<span
					key={`${section.frame}:${section.label}`}
					className="timecode-suggested-section"
					style={{ left: timelineFrameX(section.frame, pixelsPerFrame) }}
				>
					<b>{section.label}</b>
				</span>
			))}
		</div>
	);
}

function medianGap(frames: readonly number[]) {
	if (frames.length < 2) return Number.POSITIVE_INFINITY;
	const gaps = frames.slice(1).map((frame, index) => frame - frames[index]).sort((a, b) => a - b);
	return gaps[gaps.length >> 1];
}

/** The audio lane's own entry to song analysis, so the strip above takes no room until used. */
export interface SongAnalysisLaneControl {
	label: string;
	ariaLabel: string;
	onPress(): void;
}

const LaneControlContext = createContext<SongAnalysisLaneControl | undefined>(undefined);

export const SongAnalysisLaneControlProvider = LaneControlContext.Provider;

export function SongAnalysisLaneButton() {
	const control = useContext(LaneControlContext);
	if (!control) return null;
	return (
		<Button
			className="timecode-song-analysis-lane"
			size="compact"
			aria-label={control.ariaLabel}
			onPointerDown={(event) => event.stopPropagation()}
			onClick={control.onPress}
		>
			{control.label}
		</Button>
	);
}
