import { HEADER, ENVELOPE, NAME_TOP, PEAK_Y, BASE_Y } from "./cueEditorLayout";
import { Button } from "@tosklight/ui";
import type { CSSProperties } from "react";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { TIMECODE_FPS, type CueClipTimingRow } from "./cueClipTiming";
import { formatFrame } from "./timecodeEditorShared";

export { envelopePoints } from "./cueEnvelopeGeometry";
import { envelopePoints } from "./cueEnvelopeGeometry";

export function CueEnvelope({
	row,
	index,
	isSelected,
	expanded = false,
	last,
	position,
	width,
	clip,
	onSelectCue,
}: {
	row: CueClipTimingRow;
	index: number;
	isSelected: boolean;
	expanded?: boolean;
	last: boolean;
	position(frame: number): number;
	width: number;
	clip: TimecodeCueListClip;
	onSelectCue?(id: string): void;
}) {
	const left = position(row.startFrame);
	const right = position(last ? clip.end_frame : row.outFade.endFrame);
	const label = `Cue ${row.cue.number} ${row.cue.name || "Untitled"}`;
	const inStart = position(row.inFade.startFrame);
	const inEnd = position(row.inFade.endFrame);
	const outStart = last
		? Math.max(width, inEnd)
		: position(row.outFade.startFrame);
	const outEnd = last ? outStart : right;
	const base = BASE_Y - HEADER;
	const peak = PEAK_Y - HEADER;
	const points = envelopePoints(
		left,
		inStart,
		inEnd,
		outStart,
		outEnd,
		base,
		peak,
		0,
		width,
	);
	return (
		<span
			key={row.cue.id ?? index}
			className={`timecode-cue-timing ${isSelected ? "selected" : ""} ${row.diagnostic ? "unsupported" : ""}`}
			style={{ "--cue-hue": `${(index * 43 + 190) % 360}` } as CSSProperties}
		>
			{expanded && (
				<svg
					className="timecode-cue-envelope"
					width={width}
					height={ENVELOPE}
					aria-hidden="true"
				>
					<polygon points={points} />
					{isSelected && (
						<>
							<polyline
								className="timecode-cue-fade-guide in"
								points={`${inStart},${base} ${inEnd},${peak}`}
							/>
							{!last && (
								<polyline
									className="timecode-cue-fade-guide out"
									points={`${outStart},${peak} ${outEnd},${base}`}
								/>
							)}
						</>
					)}
				</svg>
			)}
			{expanded && (
				<Button
					variant="ghost"
					size="compact"
					type="button"
					className="timecode-cue-envelope-hit"
					aria-label={label}
					aria-pressed={isSelected}
					style={{
						left,
						width: Math.max(20, Math.min(width, right) - left),
						height: ENVELOPE,
						clipPath: `polygon(0 100%, ${Math.max(0, inEnd - left)}px 0, 100% 0, 100% 100%)`,
					}}
					onPointerDown={(event) => event.stopPropagation()}
					onClick={(event) => {
						event.stopPropagation();
						if (row.cue.id) onSelectCue?.(row.cue.id);
					}}
				/>
			)}
			<CueTriggerBoundary row={row} left={left} selected={isSelected} />
			<Button
				variant="ghost"
				size="compact"
				type="button"
				className={`timecode-cue-name ${expanded ? "" : "timecode-cue-block"}`}
				contentAlign="left"
				aria-label={`Edit ${label}`}
				aria-pressed={isSelected}
				style={{
					left,
					width: Math.max(
						1,
						Math.min(width, position(row.handoverFrame)) - left,
					),
					top: expanded ? NAME_TOP : 3,
				}}
				onPointerDown={(event) => event.stopPropagation()}
				onClick={(event) => {
					event.stopPropagation();
					if (row.cue.id) onSelectCue?.(row.cue.id);
				}}
			>
				<span className="timecode-cue-block-title">
					<b>{row.cue.number}</b> {row.cue.name || "Untitled"}
				</span>
				{!expanded && (
					<small className="timecode-cue-block-trigger">
						{formatFrame(row.startFrame, TIMECODE_FPS)}
					</small>
				)}
			</Button>
		</span>
	);
}

function CueTriggerBoundary({
	row,
	left,
	selected,
}: {
	row: CueClipTimingRow;
	left: number;
	selected: boolean;
}) {
	return (
		<span
			className={`timecode-cue-trigger-boundary ${selected ? "selected" : ""}`}
			data-trigger-frame={row.startFrame}
			style={{ left }}
			title={`Cue ${row.cue.number} trigger · ${formatFrame(row.startFrame, TIMECODE_FPS)}`}
			aria-hidden="true"
		></span>
	);
}
