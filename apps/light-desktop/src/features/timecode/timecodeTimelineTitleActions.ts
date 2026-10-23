import { createElement, type RefObject } from "react";
import type { TitleAction } from "@tosklight/ui";
import type { TimecodeTimelineEditorHandle } from "./TimecodeTimelineEditor";
import type { TimelineTitleState } from "./useTimelineTitleActions";
function keyframeIcon(direction: -1 | 1) {
 return createElement("svg", { width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, "aria-hidden": true },
  createElement("title", null, direction < 0 ? "Previous keyframe" : "Next keyframe"),
  createElement("path", { d: direction < 0 ? "M5 5v14M15 6l-6 6 6 6M19 12H9" : "M19 5v14M9 6l6 6-6 6M5 12h10", strokeLinecap: "round", strokeLinejoin: "round" }));
}
export function timecodeTimelineTitleActions(ref: RefObject<TimecodeTimelineEditorHandle | null>, state: TimelineTitleState): TitleAction[] {
 return [
  { id: "previous-keyframe", icon: keyframeIcon(-1), ariaLabel: "Prev Keyframe", disabled: !state.canStep, onPress: () => ref.current?.stepKeyframe(-1) },
  { id: "next-keyframe", icon: keyframeIcon(1), ariaLabel: "Next Keyframe", disabled: !state.canStep, onPress: () => ref.current?.stepKeyframe(1) },
  { id: "fit-timeline", label: createElement("span", { title: "Fit all frames into view without changing timing" }, "Fit timeline"), ariaLabel: "Fit timeline", onPress: () => ref.current?.fit() },
  ...(state.canInsert ? [{ id: "insert-keyframe", label: "Insert Keyframe", onPress: () => ref.current?.insertKeyframe() }] : []),
  ...(state.canDeleteKeyframe ? [{ id: "delete-keyframe", label: "Delete Keyframe", onPress: () => ref.current?.deleteSelection() }] : []),
  ...(state.canAddClip ? [{ id: "add-clip", label: "Add clip", onPress: () => ref.current?.addClip() }] : []),
  ...(state.canDeleteClip ? [{ id: "delete-clip", label: "Delete clip", onPress: () => ref.current?.deleteSelection() }] : []),
 ];
}
