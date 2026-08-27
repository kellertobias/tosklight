import type { PointerEvent as ReactPointerEvent } from "react";
import { useMemo } from "react";
import type { TimecodeLane } from "../../api/types/timecode";
import { Button } from "@tosklight/ui";

/// Height of the waveform's viewBox. The envelope is stretched to the lane, so this is only the
/// unit the path is written in.
const WAVEFORM_HEIGHT = 48;

export function Waveform({ peaks }: { peaks: readonly number[] }) {
	// The waveform is the surface markers get aligned against, so it is drawn at high resolution.
	// One element per bucket would be thousands of nodes; the envelope is a single path instead.
	//
	// Only the upper half is drawn: audio is symmetric about its own zero, so the lower half
	// repeats the upper one and costs half the lane to say nothing. The envelope therefore stands
	// on the lane floor and grows upward, which also puts its baseline exactly on the lane's
	// bottom edge rather than floating the whole shape above the lane.
	const outline = useMemo(() => {
		if (!peaks.length) return "";
		const points: string[] = [];
		for (let index = 0; index < peaks.length; index += 1) {
			points.push(`${index} ${WAVEFORM_HEIGHT - peaks[index] * WAVEFORM_HEIGHT}`);
		}
		points.push(`${peaks.length - 1} ${WAVEFORM_HEIGHT}`);
		points.push(`0 ${WAVEFORM_HEIGHT}`);
		return `M ${points.join(" L ")} Z`;
	}, [peaks]);
	return (
		<svg
			viewBox={`0 0 ${Math.max(peaks.length, 1)} ${WAVEFORM_HEIGHT}`}
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			<path className="timecode-waveform-envelope" d={outline} />
		</svg>
	);
}

/// A lane's name and what it is showing, and the grip that selects or reorders it.
export function LaneLabel({
	lane,
	audioFileName,
	startLaneDrag,
	consumeLaneDragClick,
	onSelectLane,
}: {
	lane: TimecodeLane;
	audioFileName?: string | null;
	startLaneDrag(event: ReactPointerEvent, laneId: string): void;
	consumeLaneDragClick(laneId: string): boolean;
	onSelectLane(laneId: string): void;
}) {
	const kind = lane.content.kind.replaceAll("_", " ");
	return (
		<div className="timecode-editor-lane-label">
			<Button
				className="timecode-lane-select"
				aria-label={`${lane.name} · ${kind}. Drag to reorder lane`}
				onPointerDown={(event) => startLaneDrag(event, lane.id)}
				onClick={() => {
					if (!consumeLaneDragClick(lane.id)) onSelectLane(lane.id);
				}}
			>
				<strong>{lane.name}</strong>
				{/* The audio lane's subtitle is the track it is showing, which is what an operator
				    needs to identify it; every other lane says what kind it is. */}
				{lane.content.kind === "audio_volume" && audioFileName ? (
					<span title={audioFileName}>{audioFileName}</span>
				) : (
					<span>{kind}</span>
				)}
			</Button>
		</div>
	);
}
