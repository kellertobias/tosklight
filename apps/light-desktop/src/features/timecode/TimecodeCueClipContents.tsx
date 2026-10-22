import { mergeCueTimingFields } from "./mergeCueTimingFields";
import { CueTimingInspector } from "./TimecodeCueTimingInspector";
import { CueTimingPoints } from "./TimecodeCueTimingPoints";
import { CueEnvelope } from "./TimecodeCueEnvelope";
import { formatErrorDetails } from "@tosklight/ui";
import type { PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
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
	cueFadeEditFrameBounds,
	cueTransitionFrameBounds,
} from "./cueClipTiming";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

export function CueListClipStatus({
	status,
}: {
	status: TimecodeCueListClipStatus;
}) {
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
}

interface CueFadeEditorOptions {
	cueList: TimecodeCueListOption & { body: CueList };
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	timingDefaults: CueClipTimingDefaults;
	onSaveCueList?(id: string, body: CueList): Promise<CueList>;
	onError?(message: string): void;
}

function changedCueTiming(
	body: CueList,
	row: CueClipTimingRow,
	clip: TimecodeCueListClip,
	timingDefaults: CueClipTimingDefaults,
	kind: CueFadeKind,
	edge: CueFadeEdge,
	frame: number,
): CueList {
	const cue = body.cues.find((candidate) => candidate.id === row.cue.id);
	if (!cue) throw new Error("The Cue is no longer available.");
	const bounds = cueFadeEditFrameBounds(
		clip,
		body,
		timingDefaults,
		row,
		kind,
		edge,
	);
	if (!bounds) {
		throw new Error(
			"This timing point cannot move without changing Cue order.",
		);
	}
	const target = Math.max(
		bounds.minimum,
		Math.min(bounds.maximum, Math.round(frame)),
	);
	const result = cueWithDraggedFade(cue, row, clip, kind, edge, target);
	if (!result.cue) {
		throw new Error(result.error ?? "Cue timing is invalid.");
	}
	return {
		...body,
		cues: body.cues.map((value) => (value.id === cue.id ? result.cue! : value)),
	};
}

function useCueFadeDrag({
	cueList,
	clip,
	pixelsPerFrame,
	timingDefaults,
	onSaveCueList,
	onError,
}: CueFadeEditorOptions) {
	const [preview, setPreview] = useState<CueList | null>(null);
	const [dragError, setDragError] = useState("");
	const [saving, setSaving] = useState(false);
	const drag = useRef<CueFadeDrag | null>(null);
	const activeBody = preview ?? cueList.body;
	useEffect(() => {
		if (!drag.current) setPreview(null);
	}, [cueList.body]);
	const changed = (
		body: CueList,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
		frame: number,
	) => {
		try {
			return changedCueTiming(
				body,
				row,
				clip,
				timingDefaults,
				kind,
				edge,
				frame,
			);
		} catch (reason) {
			setDragError(formatErrorDetails(reason));
			return null;
		}
	};
	const save = async (basis: CueList, proposed: CueList, label: string) => {
		if (!onSaveCueList) return;
		setSaving(true);
		setDragError("");
		try {
			const next = mergeCueTimingFields(cueList.body, basis, proposed);
			const ordered = cueClipTimingRows(clip, next, timingDefaults).rows;
			if (
				ordered.some(
					(row, index) =>
						index > 0 && row.startFrame < ordered[index - 1].startFrame,
				)
			)
				throw new Error(
					"Cue triggers changed during this edit. The timing edit would cross a neighbouring Cue.",
				);
			setPreview(next);
			await onSaveCueList(cueList.id, next);
			setPreview(null);
		} catch (reason) {
			const message = `${label} was not saved: ${formatErrorDetails(reason)}`;
			setPreview(null);
			setDragError(message);
			onError?.(message);
		} finally {
			setSaving(false);
		}
	};
	useEffect(() => {
		const move = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || current.pointerId !== event.pointerId) return;
			const next = changed(
				current.body,
				current.row,
				current.kind,
				current.edge,
				current.startHandleFrame +
					Math.round((event.clientX - current.startX) / pixelsPerFrame),
			);
			current.next = next;
			if (next) setPreview(next);
		};
		const finish = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || current.pointerId !== event.pointerId) return;
			drag.current = null;
			if (event.type === "pointercancel" || !current.next) {
				setPreview(null);
				return;
			}
			void save(
				current.body,
				current.next,
				`Cue ${current.row.cue.number} ${current.kind === "in" ? "In" : "Out"} fade`,
			);
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", finish);
		window.addEventListener("pointercancel", finish);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", finish);
			window.removeEventListener("pointercancel", finish);
		};
	});
	const begin = (
		event: ReactPointerEvent,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
	) => {
		event.preventDefault();
		event.stopPropagation();
		if (saving || row.diagnostic || !onSaveCueList) return;
		(event.currentTarget as HTMLElement).focus({ preventScroll: true });
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
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};
	const commit = (
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
		frame: number,
	) => {
		if (saving || !onSaveCueList) return;
		const next = changed(activeBody, row, kind, edge, frame);
		if (next)
			void save(
				activeBody,
				next,
				`Cue ${row.cue.number} ${kind === "in" ? "In" : "Out"} fade`,
			);
	};
	return { activeBody, begin, commit, dragError, saving };
}

export function CueListClipContents({
	cueList,
	clip,
	pixelsPerFrame,
	timingDefaults,
	onSaveCueList,
	onPlaceCueStart,
	onMoveClip,
	onSetClipFade,
	onError,
	selectedCueId = null,
	onSelectCue,
	expanded = false,
	inspectorHost = null,
}: {
	cueList: TimecodeCueListOption & { body: CueList };
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	timingDefaults: CueClipTimingDefaults;
	onSaveCueList?(id: string, body: CueList): Promise<CueList>;
	onPlaceCueStart?(clipId: string, cueId: string, offset: number): void;
	onMoveClip?(clipId: string, startFrame: number): void;
	onSetClipFade?(clipId: string, kind: "in" | "out", frames: number): void;
	onError?(message: string): void;
	selectedCueId?: string | null;
	onSelectCue?(cueId: string): void;
	expanded?: boolean;
	inspectorHost?: HTMLElement | null;
}) {
	const [triggerPreview, setTriggerPreview] = useState<number | null>(null);
	const { activeBody, begin, commit, dragError, saving } = useCueFadeDrag({
		cueList,
		clip,
		pixelsPerFrame,
		timingDefaults,
		onSaveCueList,
		onError,
	});
	const projected = cueClipTimingRows(clip, activeBody, timingDefaults);
	const selected = projected.rows.find((row) => row.cue.id === selectedCueId);
	const position = (frame: number) =>
		Math.max(0, frame - clip.start_frame) * pixelsPerFrame;
	const width = (clip.end_frame - clip.start_frame) * pixelsPerFrame;
	const triggerBounds = selected?.cue.id
		? cueTransitionFrameBounds(
				clip,
				activeBody,
				timingDefaults,
				selected.cue.id,
			)
		: null;
	const first = selected === projected.rows[0];
	const canMoveTrigger = Boolean(
		selected && (first ? onMoveClip : triggerBounds && onPlaceCueStart),
	);
	const placeTrigger = (frame: number) => {
		if (!selected?.cue.id) return;
		if (first) onMoveClip?.(clip.id, Math.max(0, frame));
		else if (triggerBounds)
			onPlaceCueStart?.(
				clip.id,
				selected.cue.id,
				Math.max(
					triggerBounds.minimum,
					Math.min(triggerBounds.maximum, frame),
				) - clip.start_frame,
			);
	};
	const source = first
		? "Clip start · moves the clip"
		: selected?.transition
			? selected.transition === "placed"
				? "Manual GO · placed in this clip"
				: "Manual GO · follows previous completion until placed"
			: `${selected?.cue.trigger.type ?? "Automatic"} · derived from Cuelist trigger settings`;

	return (
		<>
			<span
				className={`timecode-cue-clip-contents ${expanded ? "expanded" : ""}`}
				aria-busy={saving}
			>
				{projected.rows.map((row, index) => {
					return (
						<CueEnvelope
							key={row.cue.id ?? index}
							row={row}
							index={index}
							isSelected={expanded && row.cue.id === selectedCueId}
							last={index === projected.rows.length - 1}
							position={position}
							width={width}
							clip={clip}
							onSelectCue={onSelectCue}
						/>
					);
				})}
				{selected && expanded && (
					<CueTimingPoints
						cueList={activeBody}
						timingDefaults={timingDefaults}
						selected={selected}
						clip={clip}
						position={position}
						pixelsPerFrame={pixelsPerFrame}
						triggerPreview={triggerPreview}
						first={first}
						triggerBounds={triggerBounds}
						canMoveTrigger={canMoveTrigger}
						setTriggerPreview={setTriggerPreview}
						placeTrigger={placeTrigger}
						saving={saving}
						editable={Boolean(onSaveCueList)}
						begin={begin}
						commit={commit}
					/>
				)}
				{(projected.error || dragError || selected?.diagnostic) && (
					<span className="timecode-cue-clip-diagnostic" role="status">
						{dragError || projected.error || selected?.diagnostic}
					</span>
				)}
			</span>
			{inspectorHost && selected && expanded
				? createPortal(
						<CueTimingInspector
							key={`${clip.id}:${selected.cue.id}`}
							selected={selected}
							clip={clip}
							canMoveTrigger={canMoveTrigger}
							placeTrigger={placeTrigger}
							source={source}
							saving={saving}
							onSaveCueList={onSaveCueList}
							commit={commit}
							timingDefaults={timingDefaults}
							onSetClipFade={onSetClipFade}
						/>,
						inspectorHost,
					)
				: null}
		</>
	);
}
