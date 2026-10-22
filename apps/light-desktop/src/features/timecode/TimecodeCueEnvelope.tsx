import { Button } from "@tosklight/ui";
import type { CSSProperties } from "react";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import type { CueClipTimingRow } from "./cueClipTiming";

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
	const base = expanded ? 196 : 80;
	const peak = expanded ? 28 : 12;
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
			<svg
				className="timecode-cue-envelope"
				width={width}
				height={expanded ? 202 : 86}
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
					height: expanded ? 202 : 86,
					clipPath: `polygon(0 100%, ${Math.max(0, inEnd - left)}px 0, 100% 0, 100% 100%)`,
				}}
				onPointerDown={(event) => event.stopPropagation()}
				onClick={(event) => {
					event.stopPropagation();
					if (row.cue.id) onSelectCue?.(row.cue.id);
				}}
			/>
			<Button
				variant="ghost"
				size="compact"
				type="button"
				className="timecode-cue-name"
				contentAlign="left"
				aria-label={`Edit ${label}`}
				aria-pressed={isSelected}
				style={{
					left,
					width: Math.max(1, position(row.handoverFrame) - left),
					top: expanded ? 234 : 112,
				}}
				onPointerDown={(event) => event.stopPropagation()}
				onClick={(event) => {
					event.stopPropagation();
					if (row.cue.id) onSelectCue?.(row.cue.id);
				}}
			>
				<b>{row.cue.number}</b> {row.cue.name || "Untitled"}
			</Button>
			{last && (
				<span className="timecode-cue-end-marker" style={{ left: width }}>
					{clip.end_behavior === "hold" ? "Hold" : "Release"}
					{clip.out_fade_frames > 0 ? " · Clip Out" : ""}
				</span>
			)}
		</span>
	);
}
