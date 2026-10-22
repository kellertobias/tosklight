import { createContext, useContext, type ReactNode } from "react";
import { NumberField } from "@tosklight/ui";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";

export type ClipResizeSelection = TimecodeEditorSelection & { kind: "clip" };
export type ResizeClipBoundary = (selection: ClipResizeSelection, edge: "start" | "end", frame: number) => void;
interface BoundaryControl { frame: number; minimum: number; maximum: number; change?: (frame: number) => void }
export interface CueClipBoundarySlots { inControl: ReactNode; outControl: ReactNode }
const Context = createContext<CueClipBoundarySlots | null>(null);
export const CueClipBoundaryProvider = Context.Provider;
export function useCueClipBoundarySlots() { return useContext(Context); }

/** Build controls from the current Timecode while keeping writes on its existing resize path. */
export function cueClipBoundarySlots({ definition, selection, onResizeClip }: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	onResizeClip?: ResizeClipBoundary;
}): CueClipBoundarySlots | null {
	if (selection?.kind !== "clip") return null;
	const chosen = { ...selection, kind: "clip" as const };
	const lane = definition.lanes.find((candidate) => candidate.id === selection.laneId);
	if (lane?.content.kind !== "cue_list") return null;
	const clips = lane.content.clips;
	const clip = clips.find((candidate) => candidate.id === selection.itemId);
	if (!clip) return null;
	const minimum = Math.max(0, ...clips.filter((other) => other.id !== clip.id && other.end_frame <= clip.start_frame).map((other) => other.end_frame));
	const maximum = Math.min(definition.duration_frame ?? Infinity, ...clips.filter((other) => other.id !== clip.id && other.start_frame >= clip.end_frame).map((other) => other.start_frame));
	return {
		inControl: <BoundaryField key={`${clip.id}-start`} label="Start frame" frame={clip.start_frame} minimum={minimum} maximum={clip.end_frame - 1} change={onResizeClip ? (frame) => onResizeClip(chosen, "start", frame) : undefined} />,
		outControl: <BoundaryField key={`${clip.id}-end`} label="End frame" frame={clip.end_frame} minimum={clip.start_frame + 1} maximum={maximum} change={onResizeClip ? (frame) => onResizeClip(chosen, "end", frame) : undefined} />,
	};
}
function BoundaryField({ label, frame, minimum, maximum, change }: BoundaryControl & { label: string }) {
	return <NumberField label={label} value={frame} min={minimum} max={Number.isFinite(maximum) ? maximum : undefined} disabled={!change}
		onChange={(event) => {
			const text = event.currentTarget.value.trim();
			const value = Number(text);
			if (!text || !Number.isFinite(value)) return;
			const next = Math.max(minimum, Math.min(maximum, Math.round(value)));
			if (next !== frame) change?.(next);
		}} />;
}
