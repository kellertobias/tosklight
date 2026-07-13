import { useEffect, useState } from "react";
import { fixtures } from "../data/mockData";
import { SourceValue } from "../components/shared/SourceValue";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { GroupStrip } from "../components/shared/GroupStrip";
import { useApp } from "../state/AppContext";
import { fixtureTargetIds, fixtureValue } from "./fixtureVisualization";
import { GroupsPoolButton } from "../components/shared/GroupsPoolButton";
import { Button } from "../components/common";

export function FixtureSheetWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [visualization, setVisualization] = useState<VisualizationSnapshot | null>(null);
  const [preloadVisualization, setPreloadVisualization] = useState<VisualizationSnapshot | null>(null);
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
      id: index + 1,
      name: patched.definition.name ?? patched.definition.model,
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
  return (
    <div className="fixture-window">
      {!compact && <header className="window-toolbar">
        <h1>
          Fixture Sheet{" "}
          {!compact && <small>{server.selectedFixtures.length} selected</small>}
        </h1>
        <span className="source-legend">
          <i className="source-programmer">● Programmer</i>
          <i className="source-playback">● Playback</i>
          <i className="source-default">● Default</i>
        </span>
        <span className="spacer" />
        {!compact && (
          <div className="button-group">
            <GroupsPoolButton shortcutsVisible={groupsVisible} onToggleShortcuts={() => dispatch({type:"SET_BUILTIN_GROUPS_VISIBLE",window:"fixtures",value:!groupsVisible})} />
          </div>
        )}
      </header>}
      <div className="fixture-table">
        <div className="fixture-row fixture-head">
          <span>ID</span>
          <span>Name / type</span>
          <span>Dimmer</span>
          <span>Color</span>
          <span>Position</span>
          <span>Beam</span>
          <span>Focus</span>
        </div>
        {visible.map((fixture) => (
          <div className={`fixture-row-shell ${fixture.fixtureId && server.selectedFixtures.includes(fixture.fixtureId) ? "selected" : ""}`} key={fixture.fixtureId || fixture.id}>
          <Button
            onClick={() =>
              fixture.fixtureId && void server.setSelection([fixture.fixtureId])
            }
            className="fixture-row"
          >
            <span>{fixture.id}</span>
            <span className="fixture-name">
              <b>{fixture.name}</b>
              <small>{fixture.type}</small>
              {fixture.limitingGroups.length > 0 && (
                <em
                  title={fixture.limitingGroups
                    .map(
                      (group) =>
                        `${group.body.name}: ${Math.round((group.body.master ?? 1) * 100)}%`,
                    )
                    .join(", ")}
                >
                  ◒ Group master{" "}
                  {Math.round(
                    Math.max(
                      ...fixture.limitingGroups.map(
                        (group) => group.body.master ?? 1,
                      ),
                    ) * 100,
                  )}
                  %
                </em>
              )}
            </span>
            <SourceValue source={fixture.sources.dimmer}>
              <i className="vertical-meter">
                <i style={{ height: `${fixture.dimmer}%` }} />
              </i>
              {fixture.dimmer}%
              {fixture.preloadDimmer != null && <small className="preload-value">→ {fixture.preloadDimmer}%</small>}
            </SourceValue>
            <SourceValue source={fixture.sources.color}>
              <i className="color-dot" style={{ background: fixture.color }} />
              {fixture.colorLabel}
              {fixture.preloadColor && <small className="preload-value"><i className="color-dot" style={{ background: fixture.preloadColor }} /> Preload</small>}
            </SourceValue>
            <SourceValue source={fixture.sources.position}>
              <i className="position-glyph">
                <i
                  style={{
                    left: `${fixture.pan % 75}%`,
                    top: `${fixture.tilt % 65}%`,
                  }}
                />
              </i>
              {fixture.positionLabel ?? `${fixture.pan}° / ${fixture.tilt}°`}
              {fixture.preloadPan != null && fixture.preloadTilt != null && <small className="preload-value">→ {fixture.preloadPan} / {fixture.preloadTilt}</small>}
            </SourceValue>
            <SourceValue source={fixture.sources.beam}>
              {fixture.beam}
            </SourceValue>
            <SourceValue source={fixture.sources.focus}>
              {fixture.focus}
            </SourceValue>
          </Button>
          </div>
        ))}
        {Array.from({ length: Math.max(0, (compact ? 12 : 24) - visible.length) }, (_, index) => <div className="fixture-row fixture-empty-row" aria-hidden="true" key={`empty-row-${index}`}><span>{visible.length + index + 1}</span><span /><span /><span /><span /><span /><span /></div>)}
      </div>
      {groupsVisible && <GroupStrip />}
    </div>
  );
}
