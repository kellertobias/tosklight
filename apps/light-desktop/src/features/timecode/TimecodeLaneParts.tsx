import type { PointerEvent as ReactPointerEvent } from "react";
import { useMemo } from "react";
import type { TimecodeLane } from "../../api/types/timecode";
import { Button } from "@tosklight/ui";

/// Height of the waveform's viewBox. The envelope is stretched to the lane, so this is only the
/// unit the path is written in.
const WAVEFORM_HEIGHT = 48;

function round(value: number) {
	return Math.round(value * 10) / 10;
}

export function Waveform({ peaks }: { peaks: readonly number[] }) {
	// The waveform is the surface markers get aligned against, so it is drawn at high resolution.
	// One element per bucket would be thousands of nodes; the envelope is a single path instead.
	//
	// The envelope is symmetric about the lane's own centre line, the way a recorded signal is
	// symmetric about its zero, and it is filled rather than outlined so the body of the audio
	// carries the colour. An outline left the loud passages as empty space bounded by a hairline,
	// which is the hardest part of the lane to read at a glance.
	const outline = useMemo(() => {
		if (!peaks.length) return "";
		// A quiet recording is still a waveform. Normalising against the track's own loudest
		// bucket makes that peak fill the lane, so the shape stays legible whatever the take was
		// mastered at; a silent track has no peak to normalise against and stays flat.
		const loudest = peaks.reduce((peak, value) => Math.max(peak, value), 0);
		const scale = loudest > 0 ? 1 / loudest : 0;
		const middle = WAVEFORM_HEIGHT / 2;
		const upper: string[] = [];
		const lower: string[] = [];
		for (let index = 0; index < peaks.length; index += 1) {
			const half = Math.min(1, peaks[index] * scale) * middle;
			// Thousands of buckets are written into one attribute, so each coordinate is rounded
			// to a tenth of a viewBox unit rather than carrying float noise no lane can show.
			upper.push(`${index} ${round(middle - half)}`);
			lower.push(`${index} ${round(middle + half)}`);
		}
		lower.reverse();
		return `M ${upper.concat(lower).join(" L ")} Z`;
	}, [peaks]);
	return (
		<svg
			className="timecode-waveform"
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
