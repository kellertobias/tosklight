import { useApp } from "../../state/AppContext";
import { TouchTimeSurface } from "./TouchTimeSurface";

const speeds = { A: 120, B: 90, C: 60, D: 30 } as const;
export function PlaybackTools() {
  const { state, dispatch } = useApp();
  return <div className="playback-tools"><div className="speed-group-stack">{Object.entries(speeds).map(([group, bpm]) => <button className={state.speedGroup === group ? "active" : ""} key={group} onClick={() => dispatch({ type: "SET_SPEED_GROUP", value: group as keyof typeof speeds })}>{group} · {bpm}</button>)}</div><TouchTimeSurface label="Programmer fade" value={state.programmerFade} maximum={20} display={`${state.programmerFade.toFixed(1)} s`} onChange={(value) => dispatch({ type: "SET_PROGRAMMER_FADE", value })}/><TouchTimeSurface label="Sequence master" value={state.sequenceMaster} maximum={200} display={`${state.sequenceMaster}%`} onChange={(value) => dispatch({ type: "SET_SEQUENCE_MASTER", value })}/></div>;
}
