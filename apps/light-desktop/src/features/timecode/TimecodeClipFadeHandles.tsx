import type { PointerEvent as ReactPointerEvent } from "react";
import { useRef, useState } from "react";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { TIMECODE_FPS } from "./cueClipTiming";
import { formatFrame } from "./timecodeEditorShared";

export type ClipFadeKind = "in" | "out";

/// How far a fade handle sits from the clip edge it grows out of.
///
/// The in handle is measured forward from the clip start and the out handle back from the clip
/// end, so both read as the length of their own fade wherever the clip is on the timeline.
export function clipFadeFrames(
	clip: TimecodeCueListClip,
	kind: ClipFadeKind,
): number {
	return kind === "in" ? clip.in_fade_frames : clip.out_fade_frames;
}

/// Clamps a dragged fade inside the clip. Neither fade may be longer than the clip itself.
export function clampedClipFade(
	clip: TimecodeCueListClip,
	frames: number,
): number {
	const length = Math.max(0, clip.end_frame - clip.start_frame);
	return Math.round(Math.max(0, Math.min(length, frames)));
}

/// The two drag handles for a clip's global in fade and out fade.
///
/// They shape the level the clip contributes for as long as it owns its Cuelist, which is why
/// they live on the clip rather than on any Cue inside it.
export function ClipFadeHandles({
	clip,
	pixelsPerFrame,
	disabled = false,
	onCommit,
}: {
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	disabled?: boolean;
	onCommit(kind: ClipFadeKind, frames: number): void;
}) {
	const [dragged, setDragged] = useState<{
		kind: ClipFadeKind;
		frames: number;
	} | null>(null);
	const frames = (kind: ClipFadeKind) =>
		dragged?.kind === kind ? dragged.frames : clipFadeFrames(clip, kind);
	return (
		<>
			{(["in", "out"] as const).map((kind) => (
				<ClipFadeHandle
					key={kind}
					kind={kind}
					clip={clip}
					frames={frames(kind)}
					pixelsPerFrame={pixelsPerFrame}
					disabled={disabled}
					onDrag={(value) => setDragged({ kind, frames: value })}
					onCommit={(value) => {
						setDragged(null);
						onCommit(kind, value);
					}}
				/>
			))}
		</>
	);
}

function ClipFadeHandle({
	kind,
	clip,
	frames,
	pixelsPerFrame,
	disabled,
	onDrag,
	onCommit,
}: {
	kind: ClipFadeKind;
	clip: TimecodeCueListClip;
	frames: number;
	pixelsPerFrame: number;
	disabled: boolean;
	onDrag(frames: number): void;
	onCommit(frames: number): void;
}) {
	const origin = useRef<{ x: number; frames: number } | null>(null);
	// Dragging the in handle right lengthens its fade; dragging the out handle left does the
	// same for its own, because it is measured back from the clip end.
	const direction = kind === "in" ? 1 : -1;
	const label = `Clip ${kind} fade`;
	const move = (event: ReactPointerEvent) => {
		const start = origin.current;
		if (!start) return null;
		return clampedClipFade(
			clip,
			start.frames +
				(direction * (event.clientX - start.x)) / Math.max(pixelsPerFrame, 1e-6),
		);
	};
	return (
		<span
			className={`timecode-clip-fade ${kind} ${frames > 0 ? "set" : "empty"}`}
			style={{
				[kind === "in" ? "left" : "right"]: 0,
				width: Math.max(1, frames * pixelsPerFrame),
			}}
			role="slider"
			tabIndex={disabled ? -1 : 0}
			aria-label={label}
			aria-valuemin={0}
			aria-valuemax={Math.max(0, clip.end_frame - clip.start_frame)}
			aria-valuenow={frames}
			aria-disabled={disabled || undefined}
			title={`${label} · ${formatFrame(frames, TIMECODE_FPS)}`}
			onKeyDown={(event) => {
				if (disabled) return;
				if (event.key === "ArrowLeft") {
					onCommit(clampedClipFade(clip, frames - direction));
				} else if (event.key === "ArrowRight") {
					onCommit(clampedClipFade(clip, frames + direction));
				} else return;
				event.preventDefault();
			}}
			onPointerDown={(event) => {
				if (disabled) return;
				event.preventDefault();
				event.stopPropagation();
				origin.current = { x: event.clientX, frames };
				event.currentTarget.setPointerCapture?.(event.pointerId);
			}}
			onPointerMove={(event) => {
				const next = move(event);
				if (next === null) return;
				event.stopPropagation();
				onDrag(next);
			}}
			onPointerUp={(event) => {
				const next = move(event);
				if (next === null) return;
				event.stopPropagation();
				origin.current = null;
				onCommit(next);
			}}
		/>
	);
}
