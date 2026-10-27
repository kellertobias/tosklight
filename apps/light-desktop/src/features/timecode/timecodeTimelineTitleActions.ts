import { createElement, type RefObject } from "react";
import type { TitleAction } from "@tosklight/ui";
import type { TimecodeTimelineEditorHandle } from "./TimecodeTimelineEditor";
import type { TimelineTitleState } from "./useTimelineTitleActions";
function keyframeIcon(direction: -1 | 1) {
 return createElement("svg", { width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, "aria-hidden": true },
  createElement("title", null, direction < 0 ? "Previous keyframe" : "Next keyframe"),
  createElement("path", { d: direction < 0 ? "M5 5v14M15 6l-6 6 6 6M19 12H9" : "M19 5v14M9 6l6 6-6 6M5 12h10", strokeLinecap: "round", strokeLinejoin: "round" }));
}
function keyframeEditIcon(insert: boolean) {
 return createElement("svg", { width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, "aria-hidden": true },
  createElement("title", null, insert ? "Insert keyframe" : "Delete keyframe"),
  createElement("path", { d: "M12 3 21 12 12 21 3 12Z", strokeLinejoin: "round" }),
  createElement("path", { d: insert ? "M8 12h8M12 8v8" : "M8 12h8", strokeLinecap: "round" }));
}
export function timecodeTimelineTitleActions(ref: RefObject<TimecodeTimelineEditorHandle | null>, state: TimelineTitleState): TitleAction[] {
 return [
  ...(state.canStep ? [{ id: "previous-keyframe", icon: keyframeIcon(-1), ariaLabel: "Prev Keyframe", disabled: !state.canStep, onPress: () => ref.current?.stepKeyframe(-1) },
  { id: "next-keyframe", icon: keyframeIcon(1), ariaLabel: "Next Keyframe", disabled: !state.canStep, onPress: () => ref.current?.stepKeyframe(1) }] : []),
  ...(state.canInsert ? [{ id: "insert-keyframe", icon: keyframeEditIcon(true), ariaLabel: "Insert Keyframe", onPress: () => ref.current?.insertKeyframe() }] : []),
  ...(state.canDeleteKeyframe ? [{ id: "delete-keyframe", icon: keyframeEditIcon(false), ariaLabel: "Delete Keyframe", onPress: () => ref.current?.deleteSelection() }] : []),
  ...(state.canAddClip ? [{ id: "add-clip", label: "Add clip", onPress: () => ref.current?.addClip() }] : []),
  ...(state.canDeleteClip ? [{ id: "delete-clip", label: "Delete clip", onPress: () => ref.current?.deleteSelection() }] : []),
 ];
}
