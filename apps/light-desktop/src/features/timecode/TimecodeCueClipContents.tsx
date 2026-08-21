import type { PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import type { CueList } from "../../api/types";
import type {
	TimecodeCueListClip,
	TimecodeCueListClipStatus,
} from "../../api/types/timecode";
import {
	type CueClipTimingDefaults,
	type CueClipTimingRow,
	type CueFadeEdge,
	type CueFadeKind,
	cueClipTimingRows,
	cueWithDraggedFade,
	TIMECODE_FPS,
} from "./cueClipTiming";
import { formatFrame, type TimecodeCueListOption } from "./timecodeEditorShared";

export function CueListClipStatus({ status }: { status: TimecodeCueListClipStatus }) {
	const label =
		status.state === "active"
			? "Executing"
			: status.state === "armed"
				? "Armed"
				: status.state === "held"
					? "Held"
					: status.state === "unable"
						? "Unable"
						: "Released";
	return (
		<span
			className={`timecode-cue-clip-status ${status.state}`}
			role={status.state === "unable" ? "status" : undefined}
		>
			<strong>{label}</strong>
			{status.message && <small>{status.message}</small>}
		</span>
	);
}

interface CueFadeDrag {
	pointerId: number;
	startX: number;
	startHandleFrame: number;
	row: CueClipTimingRow;
	kind: CueFadeKind;
	edge: CueFadeEdge;
	body: CueList;
	next: CueList | null;
	error: string;
}

/// Clamps a fade handle inside its legal range: a fade cannot start before its Cue,
/// cross its own opposite edge, or leave the clip.
function clampedFadeFrame(
	drag: CueFadeDrag,
	clip: TimecodeCueListClip,
	proposed: number,
): number {
	const range = drag.kind === "in" ? drag.row.inFade : drag.row.outFade;
	const lower =
		drag.edge === "end"
			? Math.max(drag.row.startFrame, range.startFrame)
			: drag.row.startFrame;
	const upper =
		drag.edge === "start"
			? Math.min(clip.end_frame, range.endFrame)
			: clip.end_frame;
	return Math.max(lower, Math.min(upper, proposed));
}

/// Owns the pointer gesture that edits Cue fade timing inside a clip. The preview
/// Cuelist is local until the pointer is released and the desk accepts the save.
function useCueFadeDrag({
	cueList,
	clip,
	pixelsPerFrame,
	onSaveCueList,
	onError,
}: {
	cueList: TimecodeCueListOption & { body: CueList };
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onError?(message: string): void;
}) {
	const [preview, setPreview] = useState<CueList | null>(null);
	const [dragError, setDragError] = useState("");
	const [saving, setSaving] = useState(false);
	const drag = useRef<CueFadeDrag | null>(null);
	const activeBody = preview ?? cueList.body;
	useEffect(() => {
		const move = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || event.pointerId !== current.pointerId) return;
			const target = clampedFadeFrame(
				current,
				clip,
				current.startHandleFrame +
					Math.round((event.clientX - current.startX) / pixelsPerFrame),
			);
			const sourceCue = current.body.cues.find(
				(cue) => cue.id === current.row.cue.id,
			);
			if (!sourceCue) return;
			const result = cueWithDraggedFade(
				sourceCue,
				current.row,
				clip,
				current.kind,
				current.edge,
				target,
			);
			if (!result.cue) {
				current.next = null;
				current.error = result.error ?? "Cue timing is invalid.";
				setDragError(current.error);
				setPreview(current.body);
				return;
			}
			const updatedCue = result.cue;
			current.next = {
				...current.body,
				cues: current.body.cues.map((cue) =>
					cue.id === updatedCue.id ? updatedCue : cue,
				),
			};
			current.error = "";
			setDragError("");
			setPreview(current.next);
		};
		const save = (current: CueFadeDrag, next: CueList) => {
			setSaving(true);
			void onSaveCueList?.(cueList.id, next)
				.then((saved) => {
					setPreview(saved);
					setDragError("");
				})
				.catch((reason) => {
					const message = `Cue ${current.row.cue.number} ${current.kind === "in" ? "In" : "Out"} fade was not saved: ${reason instanceof Error ? reason.message : String(reason)}`;
					setPreview(null);
					setDragError(message);
					onError?.(message);
				})
				.finally(() => setSaving(false));
		};
		const finish = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || event.pointerId !== current.pointerId) return;
			drag.current = null;
			if (event.type === "pointercancel") {
				setPreview(null);
				setDragError("");
				return;
			}
			if (current.error || !current.next) {
				setPreview(null);
				if (current.error) onError?.(current.error);
				return;
			}
			if (!onSaveCueList) {
				const message = "Cue timing editing is unavailable on this desk.";
				setPreview(null);
				setDragError(message);
				onError?.(message);
				return;
			}
			save(current, current.next);
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", finish);
		window.addEventListener("pointercancel", finish);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", finish);
			window.removeEventListener("pointercancel", finish);
		};
	}, [clip, cueList.id, onError, onSaveCueList, pixelsPerFrame]);
	const begin = (
		event: ReactPointerEvent,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
	) => {
		event.preventDefault();
		event.stopPropagation();
		if (saving || row.diagnostic) return;
		const range = kind === "in" ? row.inFade : row.outFade;
		drag.current = {
			pointerId: event.pointerId,
			startX: event.clientX,
			startHandleFrame: edge === "start" ? range.startFrame : range.endFrame,
			row,
			kind,
			edge,
			body: structuredClone(activeBody),
			next: null,
			error: "",
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};
	return { activeBody, begin, dragError, saving };
}

function CueTimingRow({
	row,
	rows,
	clip,
	pixelsPerFrame,
	saving,
	position,
	transitionFrame,
	onTransitionDrag,
	onPlaceCueStart,
	onBeginFade,
}: {
	row: CueClipTimingRow;
	rows: readonly CueClipTimingRow[];
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	saving: boolean;
	position(frame: number): number;
	transitionFrame?: number;
	onTransitionDrag(cueId: string, frame: number | null): void;
	onPlaceCueStart?(clipId: string, cueId: string, offsetFrame: number): void;
	onBeginFade(
		event: ReactPointerEvent,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
	): void;
}) {
	const cueId = row.cue.id;
	/// A transition may not run past the clip or back over the preceding Cue.
	const place = (frame: number) => {
		if (!cueId) return;
		const previous = rows[rows.indexOf(row) - 1];
		const minimum = Math.max(
			clip.start_frame,
			(previous?.startFrame ?? clip.start_frame) + 1,
		);
		const maximum = Math.max(clip.start_frame + 1, clip.end_frame - 1);
		const clamped = Math.round(Math.max(minimum, Math.min(maximum, frame)));
		onTransitionDrag(cueId, null);
		onPlaceCueStart?.(clip.id, cueId, clamped - clip.start_frame);
	};
	return (
		<span
			className={`timecode-cue-timing ${row.diagnostic ? "unsupported" : ""}`}
		>
			<span
				className="timecode-cue-start-marker"
				style={{ left: position(row.startFrame) }}
				title={`Cue ${row.cue.number} start · ${formatFrame(row.startFrame, TIMECODE_FPS)}`}
			>
				{row.cue.number}
			</span>
			{row.transition && cueId && (
				<CueTransitionHandle
					cueNumber={String(row.cue.number)}
					placed={row.transition === "placed"}
					frame={transitionFrame ?? row.startFrame}
					position={position}
					disabled={!onPlaceCueStart}
					onDrag={(frame) => onTransitionDrag(cueId, frame)}
					onCommit={place}
					pixelsPerFrame={pixelsPerFrame}
				/>
			)}
			{(["in", "out"] as const).map((kind) => (
				<CueFadeRange
					key={kind}
					cueNumber={String(row.cue.number)}
					kind={kind}
					range={kind === "in" ? row.inFade : row.outFade}
					position={position}
					disabled={saving || Boolean(row.diagnostic)}
					onBegin={(event, edge) => onBeginFade(event, row, kind, edge)}
				/>
			))}
			{row.diagnostic && (
				<span className="timecode-cue-diagnostic" role="status">
					{row.diagnostic}
				</span>
			)}
		</span>
	);
}

export function CueListClipContents({
	cueList,
	clip,
	pixelsPerFrame,
	timingDefaults,
	onSaveCueList,
	onPlaceCueStart,
	onError,
}: {
	cueList: TimecodeCueListOption & { body: CueList };
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	timingDefaults: CueClipTimingDefaults;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onPlaceCueStart?(clipId: string, cueId: string, offsetFrame: number): void;
	onError?(message: string): void;
}) {
	const [transitionDrag, setTransitionDrag] = useState<{
		cueId: string;
		frame: number;
	} | null>(null);
	const { activeBody, begin, dragError, saving } = useCueFadeDrag({
		cueList,
		clip,
		pixelsPerFrame,
		onSaveCueList,
		onError,
	});
	const projected = cueClipTimingRows(clip, activeBody, timingDefaults);
	const clipLength = Math.max(1, clip.end_frame - clip.start_frame);
	const position = (frame: number) =>
		Math.max(0, Math.min(clipLength, frame - clip.start_frame)) *
		pixelsPerFrame;
	return (
		<span className="timecode-cue-clip-contents" aria-busy={saving}>
			{projected.error && (
				<span className="timecode-cue-clip-diagnostic" role="status">
					{projected.error}
				</span>
			)}
			{projected.rows.map((row) => (
				<CueTimingRow
					key={row.cue.id}
					row={row}
					rows={projected.rows}
					clip={clip}
					pixelsPerFrame={pixelsPerFrame}
					saving={saving}
					position={position}
					transitionFrame={
						transitionDrag && transitionDrag.cueId === row.cue.id
							? transitionDrag.frame
							: undefined
					}
					onTransitionDrag={(cueId, frame) =>
						setTransitionDrag(frame === null ? null : { cueId, frame })
					}
					onPlaceCueStart={onPlaceCueStart}
					onBeginFade={begin}
				/>
			))}
			{dragError && (
				<span className="timecode-cue-clip-diagnostic" role="status">
					{dragError}
				</span>
			)}
		</span>
	);
}

/// The lane-owned start of a Cue that waits for a manual GO. Dragging it writes the
/// transition point into the clip, so the Cuelist itself keeps its manual trigger.
function CueTransitionHandle({
	cueNumber,
	placed,
	frame,
	position,
	pixelsPerFrame,
	disabled,
	onDrag,
	onCommit,
}: {
	cueNumber: string;
	placed: boolean;
	frame: number;
	position(frame: number): number;
	pixelsPerFrame: number;
	disabled: boolean;
	onDrag(frame: number): void;
	onCommit(frame: number): void;
}) {
	const origin = useRef<{ x: number; frame: number } | null>(null);
	return (
		<span
			className={`timecode-cue-transition ${placed ? "placed" : "default"}`}
			style={{ left: position(frame) }}
			role="slider"
			tabIndex={disabled ? -1 : 0}
			aria-label={`Cue ${cueNumber} transition point`}
			aria-valuemin={0}
			aria-valuenow={frame}
			aria-disabled={disabled || undefined}
			title={`Cue ${cueNumber} transition · ${formatFrame(frame, TIMECODE_FPS)}`}
			onKeyDown={(event) => {
				if (disabled) return;
				if (event.key === "ArrowLeft") onCommit(frame - 1);
				else if (event.key === "ArrowRight") onCommit(frame + 1);
				else return;
				event.preventDefault();
			}}
			onPointerDown={(event) => {
				if (disabled) return;
				event.preventDefault();
				event.stopPropagation();
				origin.current = { x: event.clientX, frame };
				event.currentTarget.setPointerCapture?.(event.pointerId);
			}}
			onPointerMove={(event) => {
				const start = origin.current;
				if (!start) return;
				event.stopPropagation();
				onDrag(
					start.frame + Math.round((event.clientX - start.x) / pixelsPerFrame),
				);
			}}
			onPointerUp={(event) => {
				const start = origin.current;
				if (!start) return;
				event.stopPropagation();
				origin.current = null;
				onCommit(
					start.frame + Math.round((event.clientX - start.x) / pixelsPerFrame),
				);
			}}
		/>
	);
}

function CueFadeRange({
	cueNumber,
	kind,
	range,
	position,
	disabled,
	onBegin,
}: {
	cueNumber: string;
	kind: CueFadeKind;
	range: { startFrame: number; endFrame: number };
	position(frame: number): number;
	disabled: boolean;
	onBegin(event: ReactPointerEvent, edge: CueFadeEdge): void;
}) {
	const label = kind === "in" ? "In fade" : "Out fade";
	return (
		<span
			className={`timecode-cue-fade-range ${kind}`}
			style={{
				left: position(range.startFrame),
				width: Math.max(
					2,
					position(range.endFrame) - position(range.startFrame),
				),
			}}
			title={`Cue ${cueNumber} ${label}`}
		>
			{(["start", "end"] as const).map((edge) => (
				<span
					key={edge}
					className={`timecode-cue-fade-handle ${edge}`}
					role="slider"
					tabIndex={disabled ? -1 : 0}
					aria-label={`Cue ${cueNumber} ${label} ${edge}`}
					aria-valuemin={0}
					aria-valuenow={edge === "start" ? range.startFrame : range.endFrame}
					aria-disabled={disabled || undefined}
					onPointerDown={(event) => onBegin(event, edge)}
				/>
			))}
		</span>
	);
}