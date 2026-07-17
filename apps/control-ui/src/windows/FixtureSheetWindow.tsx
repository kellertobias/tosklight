import { useEffect, useState } from "react";
import { fixtures } from "../data/mockData";
import { SourceValue } from "../components/shared/SourceValue";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { GroupStrip } from "../components/shared/GroupStrip";
import { SourceLegend } from "../components/shared/SourceLegend";
import { useApp } from "../state/AppContext";
import { DataTable, WindowHeader, WindowScrollArea, WindowSettings, type DataTableColumn } from "../components/window-kit";
import { Select, SwitchField } from "../components/common";
import { activeProgrammerFixtureIds, compareFixtureIds, cueListFixtureIds } from "./fixtureSheetFilters";
import { fixtureSheetTargets, targetHasAttribute, targetValue } from "./fixtureSheetTargets";

type FixtureOrder = "fixture-id" | "active";

export function FixtureSheetWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [visualization, setVisualization] = useState<VisualizationSnapshot | null>(null);
  const [preloadVisualization, setPreloadVisualization] = useState<VisualizationSnapshot | null>(null);
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const [fixtureOrder, setFixtureOrder] = useState<FixtureOrder>("fixture-id");
  const [activeOnly, setActiveOnly] = useState(false);
  const [cueListId, setCueListId] = useState("");
  const groupsVisible = compact ? Boolean(showGroupShortcuts) : state.fixtureGroupsVisible;
  useEffect(() => {
    let cancelled = false;
    const refresh = () => void Promise.all([server.readVisualization(), state.preload !== "idle" ? server.readVisualization(true) : Promise.resolve(null)]).then(([next, preload]) => { if (!cancelled) { setVisualization(next); setPreloadVisualization(preload); } }).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 250);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [server.readVisualization, state.preload]);
  const ownProgrammer = server.bootstrap?.active_programmers.find(
    (programmer) => programmer.session_id === server.session?.session_id,
  );
  const activeFixtureIds = activeProgrammerFixtureIds(ownProgrammer, server.groups);
  const selectedCueList = server.playbacks?.cue_lists.find((cueList) => cueList.id === cueListId);
  const cueFixtureIds = cueListFixtureIds(selectedCueList, server.groups);
  const orderedPatch = [...(server.patch?.fixtures ?? [])].sort(compareFixtureIds);
  const orderedTargets = orderedPatch
    .flatMap(fixtureSheetTargets)
    .filter((target) => !activeOnly || activeFixtureIds.has(target.fixtureId))
    .filter((target) => cueFixtureIds == null || cueFixtureIds.has(target.fixtureId))
    .sort((a, b) => {
      if (fixtureOrder === "active") {
        const activeDifference = Number(activeFixtureIds.has(b.fixtureId)) - Number(activeFixtureIds.has(a.fixtureId));
        if (activeDifference) return activeDifference;
      }
      return compareFixtureIds(a.fixture, b.fixture) || a.order - b.order;
    });
  const liveFixtures =
    orderedTargets.map((target, index) => {
      const patched = target.fixture;
      const intensity = targetValue(visualization, target, "intensity");
      const red = targetValue(visualization, target, "color.red", 1);
      const green = targetValue(visualization, target, "color.green", 1);
      const blue = targetValue(visualization, target, "color.blue", 1);
      const pan = targetValue(visualization, target, "pan");
      const tilt = targetValue(visualization, target, "tilt");
      const base = fixtures[index % fixtures.length];
      const hasIntensity = targetHasAttribute(target, "intensity");
      const hasColor = target.heads.some((head) => head.parameters.some((parameter) => parameter.attribute.startsWith("color.")));
      const hasPosition = targetHasAttribute(target, "pan") || targetHasAttribute(target, "tilt");
      const preloadIntensity = preloadVisualization && hasIntensity ? targetValue(preloadVisualization, target, "intensity") : null;
      const preloadRed = preloadVisualization && hasColor ? targetValue(preloadVisualization, target, "color.red", 1) : null;
      const preloadGreen = preloadVisualization && hasColor ? targetValue(preloadVisualization, target, "color.green", 1) : null;
      const preloadBlue = preloadVisualization && hasColor ? targetValue(preloadVisualization, target, "color.blue", 1) : null;
      const preloadPan = preloadVisualization && hasPosition ? targetValue(preloadVisualization, target, "pan") : null;
      const preloadTilt = preloadVisualization && hasPosition ? targetValue(preloadVisualization, target, "tilt") : null;
      const hasLiveColor = visualization?.values.some((entry) => entry.fixture_id === target.fixtureId && entry.attribute.startsWith("color.")) ?? false;
      const color = `rgb(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)})`;
      return ({
      ...base,
      id: target.displayId,
      name: target.name,
      type: `${patched.definition.manufacturer} · ${patched.definition.mode} · U${patched.universe}.${patched.address}`,
      fixtureId: target.fixtureId,
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
        dimmer: hasIntensity && visualization?.values.some((entry) => entry.fixture_id === target.fixtureId && entry.attribute === "intensity") ? "programmer" as const : "default" as const,
        color: hasColor && hasLiveColor ? "programmer" as const : "default" as const,
        position: hasPosition && visualization?.values.some((entry) => entry.fixture_id === target.fixtureId && (entry.attribute === "pan" || entry.attribute === "tilt")) ? "programmer" as const : "default" as const,
      },
      limitingGroups: server.groups.filter(
        (group) =>
          group.body.playback_fader != null &&
          group.body.fixtures.includes(target.fixtureId) &&
          (group.body.master ?? 1) < 1,
      ),
      positionLabel: hasPosition ? undefined : "—",
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
      {!compact && <WindowHeader title="Fixture Sheet" info={{ primary: `${server.selectedFixtures.length} selected`, secondary: <SourceLegend /> }} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
      <WindowScrollArea className="fixture-table"><DataTable columns={columns} rows={visible} rowKey={(fixture) => fixture.fixtureId || String(fixture.id)} selected={(fixture) => Boolean(fixture.fixtureId && server.selectedFixtures.includes(fixture.fixtureId))} activeIndex={activeRow} onActiveIndexChange={setActiveRow} onActivate={(fixture) => fixture.fixtureId && void server.selectionGesture({ type: "fixture", fixture_id: fixture.fixtureId })} /></WindowScrollArea>
      {groupsVisible && <GroupStrip />}
      {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="Fixture Sheet Settings" onClose={() => setSettingsAnchor(null)} tabs={[
        { id: "ordering", label: "Ordering", content: <label className="pane-option-toggle">Order fixtures <Select aria-label="Fixture sheet ordering" value={fixtureOrder} onChange={(event) => setFixtureOrder(event.target.value as FixtureOrder)}><option value="fixture-id">Fixture ID</option><option value="active">Active fixtures first</option></Select></label> },
        { id: "filters", label: "Filters", content: <><SwitchField label="Show active fixtures only" checked={activeOnly} onChange={(event) => setActiveOnly(event.target.checked)}/><label className="pane-option-toggle">Cuelist <Select aria-label="Fixture sheet Cuelist filter" value={cueListId} onChange={(event) => setCueListId(event.target.value)}><option value="">All fixtures</option>{(server.playbacks?.cue_lists ?? []).map((cueList) => <option key={cueList.id} value={cueList.id}>{cueList.name}</option>)}</Select></label></> },
        { id: "groups", label: "Groups", content: <SwitchField label="Enable group shortcuts" checked={groupsVisible} onChange={(event) => dispatch({ type: "SET_BUILTIN_GROUPS_VISIBLE", window: "fixtures", value: event.target.checked })}/> },
      ]} />}
    </div>
  );
}
