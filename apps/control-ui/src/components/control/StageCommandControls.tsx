import { useEffect } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { DualVerticalTouchFader } from "./DualVerticalTouchFader";
import type { StagePosition3d } from "../../api/ServerContext";
import { migrateStagePosition } from "../../windows/stage3dScene";
import { Button } from "../common";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

const fields: Array<{ key: keyof StagePosition3d; label: string; scale: number; offset: number }> = [
  { key: "x", label: "X Position", scale: 20, offset: 10 }, { key: "y", label: "Y Position", scale: 20, offset: 0 }, { key: "z", label: "Z Position", scale: 20, offset: 10 },
  { key: "rotationX", label: "X Rotation", scale: 360, offset: 180 }, { key: "rotationY", label: "Y Rotation", scale: 360, offset: 180 }, { key: "rotationZ", label: "Z Rotation", scale: 360, offset: 180 },
];

export function StageCommandControls() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const hardwareConnected = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const selected = server.selectedFixtures;
  const positions = Object.fromEntries((server.patch?.fixtures ?? []).map((fixture, index) => [fixture.fixture_id, server.stageLayout?.body.positions3d?.[fixture.fixture_id] ?? migrateStagePosition(server.stageLayout?.body.positions?.[fixture.fixture_id], index)]));
  const first = positions[selected[0]];
  const update = (key: keyof StagePosition3d, nextValue: number) => {
    if (!first) return;
    const delta = nextValue - first[key];
    const nextPositions = { ...positions };
    for (const id of selected) if (nextPositions[id]) nextPositions[id] = { ...nextPositions[id], [key]: nextPositions[id][key] + delta };
    void server.saveStageLayout({ version: 2, positions: server.stageLayout?.body.positions ?? {}, positions3d: nextPositions });
  };
  useEffect(() => {
    if (!hardwareConnected) return;
    const handleEncoder = (event: Event) => {
      const { control, value } = (event as CustomEvent<{ control: string; value?: string }>).detail;
      const slot = Number(control.split("/")[1]);
      if (!["up", "down", "left", "right"].includes(value ?? "")) return;
      const direction = value === "up" || value === "right" ? 1 : -1;
      const coarse = value === "left" || value === "right";
      if (state.stageMode === "navigate") {
        if (slot === 1) dispatch({ type: "SET_STAGE_NAVIGATION", zoom: Math.max(.2, state.stageZoom + direction * (coarse ? .2 : .02)) });
        if (slot === 2) dispatch({ type: "SET_STAGE_NAVIGATION", ...(coarse ? { panY: state.stagePanY + direction * 5 } : { panX: state.stagePanX + direction * 5 }) });
        if (slot === 3 && state.stageView === "3d") dispatch({ type: "SET_STAGE_NAVIGATION", ...(coarse ? { orbitY: state.stageOrbitY + direction * 5 } : { orbitX: state.stageOrbitX + direction * 5 }) });
      } else {
        const field = fields[slot - 1];
        if (field && first) update(field.key, first[field.key] + direction * (coarse ? field.scale / 20 : field.scale / 100));
      }
    };
    window.addEventListener("light:encoder-action", handleEncoder);
    return () => window.removeEventListener("light:encoder-action", handleEncoder);
  }, [hardwareConnected, state.stageMode, state.stageView, state.stageZoom, state.stagePanX, state.stagePanY, state.stageOrbitX, state.stageOrbitY, first, selected.join("|")]);
  if (state.stageMode === "navigate") return <div className="parameter-controls stage-command-controls"><div className="family-tabs"><Button className="active">Navigate Stage</Button></div><div className="parameter-surfaces">
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
  return <div className="parameter-controls stage-command-controls"><div className="family-tabs"><Button className="active">Setup Positions</Button></div><div className="parameter-surfaces six-encoders">{fields.map((field, index) => hardwareConnected
    ? (first ? <HardwareEncoderDisplay key={field.key} slot={index + 1} target={{ label: field.label, value: `${first[field.key].toFixed(field.key.startsWith("rotation") ? 0 : 1)}${field.key.startsWith("rotation") ? "°" : " m"}`, role: "Turn · Press-turn coarse" }} editValue={first[field.key]} onEdit={(value) => update(field.key, value)} /> : <HardwareEncoderDisplay key={field.key} slot={index + 1} />)
    : <VerticalTouchFader key={field.key} label={field.label} disabled={!first} maximum={field.scale} value={(first?.[field.key] ?? 0) + field.offset} display={first ? `${first[field.key].toFixed(field.key.startsWith("rotation") ? 0 : 1)}${field.key.startsWith("rotation") ? "°" : " m"}` : "No position"} directInputOffset={field.offset} onChange={(value) => update(field.key, value - field.offset)}/>)}</div></div>;
}
