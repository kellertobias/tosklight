import { Button } from "@tosklight/ui";
import type { CSSProperties } from "react";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import type { CueClipTimingRow } from "./cueClipTiming";
export function CueEnvelope({
	row,
	index,
	isSelected,
	last,
	position,
	width,
	clip,
	onSelectCue,
}: {
	row: CueClipTimingRow;
	index: number;
	isSelected: boolean;
	last: boolean;
	position(frame: number): number;
	width: number;
	clip: TimecodeCueListClip;
	onSelectCue?(id: string): void;
}) {
	const left = position(row.startFrame);
	const right = position(last ? clip.end_frame : row.outFade.endFrame);
	const label = `Cue ${row.cue.number} ${row.cue.name || "Untitled"}`;
	const inStart = Math.min(width, position(row.inFade.startFrame));
	const inEnd = Math.min(width, position(row.inFade.endFrame));
	const outStart = last
		? width
		: Math.min(width, position(row.outFade.startFrame));
	const outEnd = Math.min(width, right);
	const points = `${left},65 ${inStart},65 ${inEnd},8 ${Math.max(inEnd, outStart)},8 ${outEnd},65`;
	return (
		<span
			key={row.cue.id ?? index}
			className={`timecode-cue-timing ${isSelected ? "selected" : ""} ${row.diagnostic ? "unsupported" : ""}`}
			style={{ "--cue-hue": `${(index * 43 + 190) % 360}` } as CSSProperties}
		>
			<svg
				className="timecode-cue-envelope"
				width={width}
				height="76"
				aria-hidden="true"
			>
				<polygon points={points} />
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
				aria-label={`Edit ${label}`}
				aria-pressed={isSelected}
				style={{
					left,
					maxWidth: Math.max(28, position(row.handoverFrame) - left - 4),
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
