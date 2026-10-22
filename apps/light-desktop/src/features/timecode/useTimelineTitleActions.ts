import { createContext, useContext, useEffect } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { deleteTimelineItem, sameSelection, type TimecodeEditorSelection } from "./editorModel";
import type { TimelineItem } from "./timecodeEditorShared";
export interface TimelineTitleState { canStep: boolean; canDeleteClip: boolean; canInsert: boolean; canDeleteKeyframe: boolean; canAddClip: boolean }
export const EMPTY_TIMELINE_TITLE_STATE: TimelineTitleState = { canStep: false, canDeleteClip: false, canInsert: false, canDeleteKeyframe: false, canAddClip: false };
export const TimelineTitleStateContext = createContext<((state: TimelineTitleState) => void) | undefined>(undefined);
export function useTimelineTitleActions({ definition, selection, laneId, items, onSelect, onCommit, onStateChange, onScrub }: {
 definition: TimecodeDefinition; selection: TimecodeEditorSelection | null; laneId: string | null; items: readonly TimelineItem[];
 onSelect(selection: TimecodeEditorSelection | null): void; onCommit(definition: TimecodeDefinition): void; onStateChange?(state: TimelineTitleState): void; onScrub?(frame: number): void;
}) {
 const inheritedStateChange = useContext(TimelineTitleStateContext);
 onStateChange ??= inheritedStateChange;
 const lane = definition.lanes.find((item) => item.id === laneId);
 const keyframes = items.filter((item) => item.laneId === laneId && (item.selection.kind === "speed" || item.selection.kind === "volume"));
 const canStep = keyframes.length > 0;
 const selectedExists = selection !== null && items.some((item) => sameSelection(item.selection, selection));
 const canDeleteClip = selectedExists && selection?.kind === "clip";
 const canAddClip = lane?.content.kind === "cue_list" || lane?.content.kind === "audio_player";
 const canInsert = lane?.content.kind === "speed_group" || lane?.content.kind === "audio_volume";
 const canDeleteKeyframe = selectedExists && (selection?.kind === "speed" || selection?.kind === "volume");
 useEffect(() => onStateChange?.({ canStep, canDeleteClip, canInsert, canDeleteKeyframe, canAddClip }), [onStateChange, canStep, canDeleteClip, canInsert, canDeleteKeyframe, canAddClip]);
 return {
  stepKeyframe(delta: -1 | 1) {
   if (!keyframes.length) return;
   const current = keyframes.findIndex((item) => sameSelection(item.selection, selection));
   const index = current < 0 ? (delta < 0 ? keyframes.length - 1 : 0) : Math.max(0, Math.min(keyframes.length - 1, current + delta));
   onSelect(keyframes[index].selection);
   onScrub?.(keyframes[index].frame);
  },
  deleteSelection() { if (selection && selectedExists) { onCommit(deleteTimelineItem(definition, selection)); onSelect(null); } },
 };
}
