import { cueBoundaryFrames } from "./cueEditorLayout";
import {
	cueClipTimingWithTriggerPreview,
	cueTriggerSource,
} from "./cueClipTimingPreview";
import { mergeCueTimingFields } from "./mergeCueTimingFields";
import { CueTimingInspector } from "./TimecodeCueTimingInspector";
import { cueTriggerEditing } from "./cueTriggerEditing";
import { CueDividerHandles } from "./TimecodeCueDividerHandles";
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
	cueClipTimingRows,
	type CueClipTimingDefaults,
	type CueClipTimingRow,
	type CueFadeEdge,
	type CueFadeKind,
	cueWithDraggedFade,
	cueFadeEditFrameBounds,
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
				? "Ready"
				: status.state === "held"
					? "Held"
					: status.state === "unable"
						? "Unable"
						: "Released";
	return (
		<span
			className={`timecode-cue-clip-status ${status.state}`}
			title={status.state === "armed" ? "Ready to run when Timecode reaches this clip" : undefined}
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

interface CueListClipContentsProps {
	cueList: TimecodeCueListOption & { body: CueList };
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	timingDefaults: CueClipTimingDefaults;
	onSaveCueList?(id: string, body: CueList): Promise<CueList>;
	onPlaceCueStart?(clipId: string, cueId: string, offset: number): void;
	onMoveClip?(clipId: string, startFrame: number): void;
	onPreviewMoveClip?(clipId: string, startFrame: number | null): void;
	onSetClipFade?(clipId: string, kind: "in" | "out", frames: number): void;
	onError?(message: string): void;
	selectedCueId?: string | null;
	onSelectCue?(cueId: string): void;
	expanded?: boolean;
	inspectorHost?: HTMLElement | null;
}

export function CueListClipContents({
	cueList,
	clip,
	pixelsPerFrame,
	timingDefaults,
	onSaveCueList,
	onPlaceCueStart,
	onMoveClip,
	onPreviewMoveClip,
	onSetClipFade,
	onError,
	selectedCueId = null,
	onSelectCue,
	expanded = false,
	inspectorHost = null,
}: CueListClipContentsProps) {
	const [triggerPreview, setTriggerPreview] = useState<number | null>(null);
	const { activeBody, begin, commit, dragError, saving } = useCueFadeDrag({
		cueList,
		clip,
		pixelsPerFrame,
		timingDefaults,
		onSaveCueList,
		onError,
	});
	const projected = cueClipTimingWithTriggerPreview(
		clip,
		activeBody,
		timingDefaults,
		selectedCueId,
		triggerPreview,
	);
	const position = (frame: number) =>
		Math.max(0, frame - clip.start_frame) * pixelsPerFrame;
	const width = (clip.end_frame - clip.start_frame) * pixelsPerFrame;
	const { selected, first, triggerBounds, canMoveTrigger, placeTrigger } =
		cueTriggerEditing(
			clip,
			activeBody,
			timingDefaults,
			projected.rows,
			selectedCueId,
			onMoveClip,
			onPlaceCueStart,
		);
	const source = cueTriggerSource(selected, first);

	return (
		<>
			<span
				className={`timecode-cue-clip-contents ${expanded ? "expanded" : ""} ${selected?.outFadeReference ? "has-out-reference" : ""}`}
				aria-busy={saving}
			>
				{projected.rows.map((row, index) => {
					return (
						<CueEnvelope
							key={row.cue.id ?? index}
							row={row}
							index={index}
							isSelected={expanded && row.cue.id === selectedCueId}
							expanded={expanded}
							last={index === projected.rows.length - 1}
							position={position}
							width={width}
							clip={clip}
							onSelectCue={onSelectCue}
						/>
					);
				})}

				{expanded && (
					<CueDividerHandles
						rows={projected.rows}
						clip={clip}
						body={activeBody}
						defaults={timingDefaults}
						position={position}
						disabled={saving}
						onSelect={onSelectCue}
						onPreviewTrigger={setTriggerPreview}
						onPreviewMoveClip={onPreviewMoveClip}
						onMoveClip={onMoveClip}
						onPlaceCueStart={onPlaceCueStart}
					/>
				)}
				{selected && expanded && (
					<CueTimingPoints
						cueList={activeBody}
						timingDefaults={timingDefaults}
						selected={selected}
						dividerFrames={cueBoundaryFrames(projected.rows)}
						clip={clip}
						position={position}
						pixelsPerFrame={pixelsPerFrame}
						triggerPreview={triggerPreview}
						first={first}
						triggerBounds={triggerBounds}
						canMoveTrigger={canMoveTrigger}
						setTriggerPreview={(frame) => {
							if (first && onPreviewMoveClip) onPreviewMoveClip(clip.id, frame);
							else setTriggerPreview(frame);
						}}
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
