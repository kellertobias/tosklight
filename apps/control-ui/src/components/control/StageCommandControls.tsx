import { useEffect } from "react";
import { useApp } from "../../state/AppContext";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { DualVerticalTouchFader } from "./DualVerticalTouchFader";
import { Button } from "../common";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

export function StageCommandControls() {
  const { state, dispatch } = useApp();
  const hardwareAttached = useHardwareConnected();
  const hardwareConnected = Boolean(hardwareAttached || state.midiProfile);
  useEffect(() => {
    if (!hardwareConnected) return;
    const handleEncoder = (event: Event) => {
      const { control, value } = (event as CustomEvent<{ control: string; value?: string }>).detail;
      const slot = Number(control.split("/")[1]);
      if (!["up", "down", "left", "right"].includes(value ?? "")) return;
      const direction = value === "up" || value === "right" ? 1 : -1;
      const coarse = value === "left" || value === "right";
      if (state.stageMode !== "navigate") return;
      if (slot === 1) dispatch({ type: "SET_STAGE_NAVIGATION", zoom: Math.max(.2, state.stageZoom + direction * (coarse ? .2 : .02)) });
      if (slot === 2) dispatch({ type: "SET_STAGE_NAVIGATION", ...(coarse ? { panY: state.stagePanY + direction * 5 } : { panX: state.stagePanX + direction * 5 }) });
      if (slot === 3 && state.stageView === "3d") dispatch({ type: "SET_STAGE_NAVIGATION", ...(coarse ? { orbitY: state.stageOrbitY + direction * 5 } : { orbitX: state.stageOrbitX + direction * 5 }) });
    };
    window.addEventListener("light:encoder-action", handleEncoder);
    return () => window.removeEventListener("light:encoder-action", handleEncoder);
  }, [hardwareConnected, state.stageMode, state.stageView, state.stageZoom, state.stagePanX, state.stagePanY, state.stageOrbitX, state.stageOrbitY]);
  if (state.stageMode !== "navigate") return null;
  return <div className="parameter-controls stage-command-controls"><div className="family-tabs"><Button className="active">Navigate Stage</Button></div><div className="parameter-surfaces">
    {hardwareConnected ? <>
      <HardwareEncoderDisplay slot={1} target={{ label: "Zoom", value: `${Math.round(state.stageZoom * 100)}%`, role: "Turn · Press-turn coarse" }} />
      <HardwareEncoderDisplay slot={2} target={{ label: "X Pan", value: String(Math.round(state.stagePanX)), role: "Turn" }} secondary={{ label: "Y Pan", value: String(Math.round(state.stagePanY)), role: "Press-turn" }} />
      {state.stageView === "3d" ? <HardwareEncoderDisplay slot={3} target={{ label: "Orbit", value: `${Math.round(state.stageOrbitX)}°`, role: "Turn" }} secondary={{ label: "Orbit tilt", value: `${Math.round(state.stageOrbitY)}°`, role: "Press-turn" }} /> : <HardwareEncoderDisplay slot={3} />}
      {[4, 5, 6].map((slot) => <HardwareEncoderDisplay key={slot} slot={slot} />)}
    </> : <>
    <VerticalTouchFader label="Zoom" value={state.stageZoom * 100} maximum={200} onChange={(value) => dispatch({ type: "SET_STAGE_NAVIGATION", zoom: Math.max(.2, value / 100) })}/>
    <DualVerticalTouchFader encoder="X/Y Pan" primary={{ label: "X Pan", value: state.stagePanX + 100, maximum: 200, display: String(Math.round(state.stagePanX)), inputOffset: 100, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", panX: value - 100 }) }} secondary={{ label: "Y Pan", value: state.stagePanY + 100, maximum: 200, display: String(Math.round(state.stagePanY)), inputOffset: 100, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", panY: value - 100 }) }}/>
    {state.stageView === "3d" && <DualVerticalTouchFader
      encoder="Orbit"
      primary={{ label: "Orbit", value: state.stageOrbitX + 180, maximum: 360, display: `${Math.round(state.stageOrbitX)}°`, inputOffset: 180, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", orbitX: value - 180 }) }}
      secondary={{ label: "Orbit tilt", value: state.stageOrbitY + 90, maximum: 180, display: `${Math.round(state.stageOrbitY)}°`, inputOffset: 90, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", orbitY: value - 90 }) }}
    />}</>}
  </div></div>;
}
