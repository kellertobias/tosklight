import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { DualVerticalTouchFader } from "./DualVerticalTouchFader";
import type { StagePosition3d } from "../../api/ServerContext";
import { migrateStagePosition } from "../../windows/stage3dScene";
import { Button } from "../common";

const fields: Array<{ key: keyof StagePosition3d; label: string; scale: number; offset: number }> = [
  { key: "x", label: "X Position", scale: 20, offset: 10 }, { key: "y", label: "Y Position", scale: 20, offset: 0 }, { key: "z", label: "Z Position", scale: 20, offset: 10 },
  { key: "rotationX", label: "X Rotation", scale: 360, offset: 180 }, { key: "rotationY", label: "Y Rotation", scale: 360, offset: 180 }, { key: "rotationZ", label: "Z Rotation", scale: 360, offset: 180 },
];

export function StageCommandControls() {
  const { state, dispatch } = useApp();
  const server = useServer();
  if (state.stageMode === "navigate") return <div className="parameter-controls stage-command-controls"><div className="family-tabs"><Button className="active">Navigate Stage</Button></div><div className="parameter-surfaces">
    <VerticalTouchFader label="Zoom" value={state.stageZoom * 100} maximum={200} onChange={(value) => dispatch({ type: "SET_STAGE_NAVIGATION", zoom: Math.max(.2, value / 100) })}/>
    <DualVerticalTouchFader encoder="X/Y Pan" primary={{ label: "X Pan", value: state.stagePanX + 100, maximum: 200, display: String(Math.round(state.stagePanX)), inputOffset: 100, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", panX: value - 100 }) }} secondary={{ label: "Y Pan", value: state.stagePanY + 100, maximum: 200, display: String(Math.round(state.stagePanY)), inputOffset: 100, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", panY: value - 100 }) }}/>
    {state.stageView === "3d" && <DualVerticalTouchFader
      encoder="Orbit"
      primary={{ label: "Orbit", value: state.stageOrbitX + 180, maximum: 360, display: `${Math.round(state.stageOrbitX)}°`, inputOffset: 180, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", orbitX: value - 180 }) }}
      secondary={{ label: "Orbit tilt", value: state.stageOrbitY + 90, maximum: 180, display: `${Math.round(state.stageOrbitY)}°`, inputOffset: 90, onChange: (value) => dispatch({ type: "SET_STAGE_NAVIGATION", orbitY: value - 90 }) }}
    />}
  </div></div>;
  const selected = server.selectedFixtures;
  const positions = Object.fromEntries((server.patch?.fixtures ?? []).map((fixture, index) => [fixture.fixture_id, server.stageLayout?.body.positions3d?.[fixture.fixture_id] ?? migrateStagePosition(server.stageLayout?.body.positions?.[fixture.fixture_id], index)]));
  const first = positions[selected[0]];
  const update = (key: keyof StagePosition3d, nextValue: number) => {
    if (!first) return;
    const delta = nextValue - first[key];
    const nextPositions = { ...positions };
    for (const id of selected) if (nextPositions[id]) nextPositions[id] = { ...nextPositions[id], [key]: nextPositions[id][key] + delta };
    void server.saveStageLayout({ version: 2, positions: server.stageLayout?.body.positions ?? {}, positions3d: nextPositions, assets: server.stageLayout?.body.assets ?? [] });
  };
  return <div className="parameter-controls stage-command-controls"><div className="family-tabs"><Button className="active">Setup Positions</Button></div><div className="parameter-surfaces six-encoders">{fields.map((field) => <VerticalTouchFader key={field.key} label={field.label} disabled={!first} maximum={field.scale} value={(first?.[field.key] ?? 0) + field.offset} display={first ? `${first[field.key].toFixed(field.key.startsWith("rotation") ? 0 : 1)}${field.key.startsWith("rotation") ? "°" : " m"}` : "No position"} directInputOffset={field.offset} onChange={(value) => update(field.key, value - field.offset)}/>)}</div></div>;
}
