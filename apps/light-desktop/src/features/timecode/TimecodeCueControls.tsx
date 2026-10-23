import { CuePointLeader } from "./TimecodeCuePointLeader";
import {
	registerFramePointer,
	routeFramePointer,
} from "./frameHandlePointerRouting";
import "./TimecodeCuePointAnchors.css";
import { TextField } from "@tosklight/ui";
import type { PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { TIMECODE_FPS } from "./cueClipTiming";
import { formatFrame } from "./timecodeEditorShared";
export function FrameHandle({
	label,
	frame,
	minimum,
	maximum,
	left,
	top,
	disabled,
	onPreview,
	onCommit,
	onBegin,
	onPointerStart,
	className = "",
	caption,
	anchorTop,
	anchorLeft,
	anchorVisual = false,
	showLeader = true,
}: {
	label: string;
	frame: number;
	minimum: number;
	maximum: number;
	left: number;
	top?: number;
	disabled?: boolean;
	onPreview?(frame: number | null): void;
	onCommit(frame: number): void;
	onBegin?(event: ReactPointerEvent): void;
	onPointerStart?(): void;
	className?: string;
	caption?: string;
	anchorTop?: number;
	anchorLeft?: number;
	anchorVisual?: boolean;
	showLeader?: boolean;
}) {
	const origin = useRef<{ pointerId: number; x: number; frame: number } | null>(
		null,
	);
	const ppf = useRef(1);
	const clamp = (value: number) =>
		Math.max(minimum, Math.min(maximum, Math.round(value)));
	const beginPointer = (event: ReactPointerEvent, target: HTMLElement) => {
		if (disabled) return;
		onPointerStart?.();
		if (onBegin) {
			onBegin(event);
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		target.focus({ preventScroll: true });
		ppf.current =
			Number(
				target
					.closest("[data-pixels-per-frame]")
					?.getAttribute("data-pixels-per-frame"),
			) || 1;
		origin.current = {
			pointerId: event.pointerId,
			x: event.clientX,
			frame,
		};
		target.setPointerCapture?.(event.pointerId);
	};
	return (
		<span
			ref={(node) =>
				registerFramePointer(node, (event) => {
					if (node) beginPointer(event, node);
				})
			}
			role="slider"
			tabIndex={disabled ? -1 : 0}
			aria-label={label}
			aria-valuemin={minimum}
			aria-valuemax={maximum}
			aria-valuenow={frame}
			data-time-anchor-x={anchorLeft ?? left}
			data-time-anchor-y={anchorTop ?? top}
			aria-valuetext={formatFrame(frame, TIMECODE_FPS)}
			aria-disabled={disabled || undefined}
			className={`timecode-cue-point ${className}`}
			style={{ left, top }}
			title={`${label} · ${formatFrame(frame, TIMECODE_FPS)}`}
			onKeyDown={(event) => {
				if (disabled || !["ArrowLeft", "ArrowRight"].includes(event.key))
					return;
				event.preventDefault();
				event.stopPropagation();
				const target = clamp(frame + (event.key === "ArrowLeft" ? -1 : 1));
				if (target !== frame) onCommit(target);
			}}
			onPointerDown={(event) => routeFramePointer(event, event.currentTarget)}
			onPointerMove={(event) => {
				const start = origin.current;
				if (!start || start.pointerId !== event.pointerId) return;
				event.stopPropagation();
				onPreview?.(
					clamp(start.frame + (event.clientX - start.x) / ppf.current),
				);
			}}
			onPointerUp={(event) => {
				const start = origin.current;
				if (!start || start.pointerId !== event.pointerId) return;
				event.stopPropagation();
				origin.current = null;
				onPreview?.(null);
				const target = clamp(
					start.frame + (event.clientX - start.x) / ppf.current,
				);
				if (target !== start.frame) onCommit(target);
			}}
			onPointerCancel={() => {
				origin.current = null;
				onPreview?.(null);
			}}
		>
			{" "}
			<span
				aria-hidden="true"
				className={anchorVisual ? "timecode-cue-point-anchor" : undefined}
				style={
					anchorVisual
						? {
								left: 16 + (anchorLeft ?? left) - left,
								top: 16 + (anchorTop ?? top ?? 0) - (top ?? 0),
								pointerEvents: "none",
							}
						: undefined
				}
			/>
			<CuePointLeader {...{ showLeader, anchorTop, top, anchorLeft, left }} />
			{caption && (
				<small className="timecode-cue-point-caption">{caption}</small>
			)}{" "}
		</span>
	);
}

export function ExactFrame({
	label,
	frame,
	disabled,
	onCommit,
}: {
	label: string;
	frame: number;
	disabled?: boolean;
	onCommit(frame: number): void;
}) {
	const [text, setText] = useState(formatFrame(frame, TIMECODE_FPS));
	const [invalid, setInvalid] = useState(false);
	const submitted = useRef<number | null>(null);
	useEffect(() => {
		setText(formatFrame(frame, TIMECODE_FPS));
		setInvalid(false);
		submitted.current = null;
	}, [frame]);
	const commit = (value = text) => {
		const parts = value.match(/^(\d+):([0-5]\d):([0-5]\d)[.:](\d{2})$/);
		if (!parts || Number(parts[4]) >= TIMECODE_FPS) {
			setInvalid(true);
			return;
		}
		const next =
			((Number(parts[1]) * 60 + Number(parts[2])) * 60 + Number(parts[3])) *
				TIMECODE_FPS +
			Number(parts[4]);
		setInvalid(false);
		if (next !== frame && submitted.current !== next) {
			submitted.current = next;
			onCommit(next);
		}
	};
	return (
		<TextField
			label={label}
			error={invalid ? "Use HH:MM:SS.FF (44 frames/second)." : undefined}
			onKeyboardCommit={(value) => commit(value)}
			aria-label={label}
			aria-invalid={invalid || undefined}
			disabled={disabled}
			value={text}
			onChange={(event) => {
				submitted.current = null;
				setText(event.target.value);
			}}
			onBlur={() => commit()}
			onKeyDown={(event) => {
				if (event.key === "Enter") {
					event.preventDefault();
					commit();
				}
			}}
		/>
	);
}
