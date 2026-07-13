import { useEffect, useState } from "react";
import { fixtures } from "../data/mockData";
import { SourceValue } from "../components/shared/SourceValue";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { GroupStrip } from "../components/shared/GroupStrip";
import { useApp } from "../state/AppContext";
import { fixtureTargetIds, fixtureValue } from "./fixtureVisualization";
import { DataTable, WindowHeader, WindowScrollArea, WindowSettings, type DataTableColumn } from "../components/window-kit";
import { Input } from "../components/common";

export function FixtureSheetWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [visualization, setVisualization] = useState<VisualizationSnapshot | null>(null);
  const [preloadVisualization, setPreloadVisualization] = useState<VisualizationSnapshot | null>(null);
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const groupsVisible = compact ? Boolean(showGroupShortcuts) : state.fixtureGroupsVisible;
  useEffect(() => {
    let cancelled = false;
    const refresh = () => void Promise.all([server.readVisualization(), state.preload !== "idle" ? server.readVisualization(true) : Promise.resolve(null)]).then(([next, preload]) => { if (!cancelled) { setVisualization(next); setPreloadVisualization(preload); } }).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 250);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [server.readVisualization, state.preload]);
  const liveFixtures =
    server.patch?.fixtures.map((patched, index) => {
      const fixtureIds = fixtureTargetIds(patched);
      const intensity = fixtureValue(visualization, patched, "intensity");
      const red = fixtureValue(visualization, patched, "color.red", 1);
      const green = fixtureValue(visualization, patched, "color.green", 1);
      const blue = fixtureValue(visualization, patched, "color.blue", 1);
      const pan = fixtureValue(visualization, patched, "pan");
      const tilt = fixtureValue(visualization, patched, "tilt");
      const preloadIntensity = preloadVisualization ? fixtureValue(preloadVisualization, patched, "intensity") : null;
      const preloadRed = preloadVisualization ? fixtureValue(preloadVisualization, patched, "color.red", 1) : null;
      const preloadGreen = preloadVisualization ? fixtureValue(preloadVisualization, patched, "color.green", 1) : null;
      const preloadBlue = preloadVisualization ? fixtureValue(preloadVisualization, patched, "color.blue", 1) : null;
      const preloadPan = preloadVisualization ? fixtureValue(preloadVisualization, patched, "pan") : null;
      const preloadTilt = preloadVisualization ? fixtureValue(preloadVisualization, patched, "tilt") : null;
      const base = fixtures[index % fixtures.length];
      const hasColor = patched.definition.heads.some((head) => head.parameters.some((parameter) => parameter.attribute.startsWith("color.")));
      const hasLiveColor = visualization?.values.some((entry) => fixtureIds.includes(entry.fixture_id) && entry.attribute.startsWith("color.")) ?? false;
      const color = `rgb(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)})`;
      return ({
      ...base,
      id: patched.fixture_number ?? index + 1,
      name: patched.name || patched.definition.name || patched.definition.model,
      type: `${patched.definition.manufacturer} · ${patched.definition.mode} · U${patched.universe}.${patched.address}`,
      fixtureId: patched.fixture_id,
      dimmer: Math.round(intensity * 100),
      color,
      colorLabel: hasColor ? color : "White",
      pan: Math.round(pan * 100),
      tilt: Math.round(tilt * 100),
      preloadDimmer: preloadIntensity == null ? null : Math.round(preloadIntensity * 100),
      preloadColor: preloadRed == null || preloadGreen == null || preloadBlue == null ? null : `rgb(${Math.round(preloadRed * 255)}, ${Math.round(preloadGreen * 255)}, ${Math.round(preloadBlue * 255)})`,
      preloadPan: preloadPan == null ? null : Math.round(preloadPan * 100),
      preloadTilt: preloadTilt == null ? null : Math.round(preloadTilt * 100),
      sources: {
        ...base.sources,
        dimmer: visualization?.values.some((entry) => fixtureIds.includes(entry.fixture_id) && entry.attribute === "intensity") ? "programmer" as const : "default" as const,
        color: hasLiveColor ? "programmer" as const : "default" as const,
        position: visualization?.values.some((entry) => fixtureIds.includes(entry.fixture_id) && (entry.attribute === "pan" || entry.attribute === "tilt")) ? "programmer" as const : "default" as const,
      },
      limitingGroups: server.groups.filter(
        (group) =>
          group.body.playback_fader != null &&
          (group.body.fixtures.includes(patched.fixture_id) ||
            patched.logical_heads.some((head) =>
              group.body.fixtures.includes(head.fixture_id),
            )) &&
          (group.body.master ?? 1) < 1,
      ),
    });}) ?? [];
  const rows = server.bootstrap
    ? liveFixtures
    : fixtures.map((fixture) => ({
        ...fixture,
        fixtureId: "",
        limitingGroups: [],
        preloadDimmer: null, preloadColor: null, preloadPan: null, preloadTilt: null,
      }));
  const visible = compact ? rows.slice(0, 12) : rows;
  const [activeRow, setActiveRow] = useState(0);
  type Row = (typeof visible)[number];
  const columns: DataTableColumn<Row>[] = [
    { id: "id", header: "ID", width: "50px", render: (fixture) => fixture.id },
    { id: "name", header: "Name / type", width: "minmax(190px,1.4fr)", render: (fixture) => <span className="fixture-name"><b>{fixture.name}</b><small>{fixture.type}</small>{fixture.limitingGroups.length > 0 && <em title={fixture.limitingGroups.map((group) => `${group.body.name}: ${Math.round((group.body.master ?? 1) * 100)}%`).join(", ")}>◒ Group master {Math.round(Math.max(...fixture.limitingGroups.map((group) => group.body.master ?? 1)) * 100)}%</em>}</span> },
    { id: "dimmer", header: "Dimmer", width: "minmax(95px,.7fr)", render: (fixture) => <SourceValue source={fixture.sources.dimmer}><i className="vertical-meter"><i style={{ height: `${fixture.dimmer}%` }} /></i>{fixture.dimmer}%{fixture.preloadDimmer != null && <small className="preload-value">→ {fixture.preloadDimmer}%</small>}</SourceValue> },
    { id: "color", header: "Color", width: "minmax(105px,1fr)", render: (fixture) => <SourceValue source={fixture.sources.color}><i className="color-dot" style={{ background: fixture.color }} />{fixture.colorLabel}{fixture.preloadColor && <small className="preload-value"><i className="color-dot" style={{ background: fixture.preloadColor }} /> Preload</small>}</SourceValue> },
    { id: "position", header: "Position", width: "minmax(145px,1.25fr)", render: (fixture) => <SourceValue source={fixture.sources.position}><i className="position-glyph"><i style={{ left: `${fixture.pan % 75}%`, top: `${fixture.tilt % 65}%` }} /></i>{fixture.positionLabel ?? `${fixture.pan}° / ${fixture.tilt}°`}{fixture.preloadPan != null && fixture.preloadTilt != null && <small className="preload-value">→ {fixture.preloadPan} / {fixture.preloadTilt}</small>}</SourceValue> },
    { id: "beam", header: "Beam", width: "minmax(80px,.8fr)", render: (fixture) => <SourceValue source={fixture.sources.beam}>{fixture.beam}</SourceValue> },
    { id: "focus", header: "Focus", width: "minmax(80px,.8fr)", render: (fixture) => <SourceValue source={fixture.sources.focus}>{fixture.focus}</SourceValue> },
  ];
  return (
    <div className="fixture-window">
      {!compact && <WindowHeader title="Fixture Sheet" info={{ primary: `${server.selectedFixtures.length} selected`, secondary: "Programmer · Playback · Default" }} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
      <WindowScrollArea className="fixture-table"><DataTable columns={columns} rows={visible} rowKey={(fixture) => fixture.fixtureId || String(fixture.id)} selected={(fixture) => Boolean(fixture.fixtureId && server.selectedFixtures.includes(fixture.fixtureId))} activeIndex={activeRow} onActiveIndexChange={setActiveRow} onActivate={(fixture) => fixture.fixtureId && void server.setSelection([fixture.fixtureId])} /></WindowScrollArea>
      {groupsVisible && <GroupStrip />}
      {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="Fixture Sheet Settings" onClose={() => setSettingsAnchor(null)} tabs={[{ id: "groups", label: "Groups", content: <label className="pane-option-toggle"><Input type="checkbox" checked={groupsVisible} onChange={(event) => dispatch({ type: "SET_BUILTIN_GROUPS_VISIBLE", window: "fixtures", value: event.target.checked })}/> Enable group shortcuts</label> }]} />}
    </div>
  );
}
