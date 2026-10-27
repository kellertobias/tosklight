import { useImperativeHandle, type ForwardedRef } from "react";
import type { TimecodeTimelineEditorHandle } from "./TimecodeTimelineEditor";
export function useTimecodeEditorHandle(ref: ForwardedRef<TimecodeTimelineEditorHandle>, { titleActions, fitTimeline, activeLaneId, frame, addKeyframe, addClip, addMarker, setSpeedGroupChooserOpen, setCueListChooserOpen, addAudioPlayerLane }: {
 titleActions: Pick<TimecodeTimelineEditorHandle, "stepKeyframe" | "deleteSelection">; fitTimeline():void; activeLaneId:string|null; frame:number;
 addKeyframe(laneId:string,frame:number):void; addClip(laneId:string):void; addMarker():void;
 setSpeedGroupChooserOpen(value:boolean):void; setCueListChooserOpen(value:boolean):void; addAudioPlayerLane(id:string):void;
}) {
 useImperativeHandle(ref, () => ({ ...titleActions, fit: fitTimeline,
  insertKeyframe: () => { if (activeLaneId) addKeyframe(activeLaneId, frame); },
  addClip: () => { if (activeLaneId) addClip(activeLaneId); }, addMarker,
  chooseSpeedLane: () => setSpeedGroupChooserOpen(true), chooseCueListLane: () => setCueListChooserOpen(true), addAudioPlayerLane,
 }));
}
