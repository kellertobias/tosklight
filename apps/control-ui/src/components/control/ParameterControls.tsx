import { useEffect, useMemo, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import type { VisualizationSnapshot } from "../../api/types";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { StageCommandControls } from "./StageCommandControls";

const families = {
  Intensity: ["intensity", "shutter", "strobe", "master"],
  Color: [
    "color.red",
    "color.green",
    "color.blue",
    "color.white",
    "color.amber",
    "color.uv",
  ],
  Position: ["pan", "tilt"],
  Beam: ["gobo", "gobo.2", "gobo.rotation", "prism", "prism.2", "iris"],
  Shapers: [
    "shaper.blade.1",
    "shaper.blade.2",
    "shaper.blade.3",
    "shaper.blade.4",
    "shaper.rotation",
  ],
  Focus: ["focus", "zoom", "frost", "edge"],
  Control: ["control.reset", "control.lamp", "control.fan", "control.mode"],
  Media: ["media.layer", "media.clip", "media.opacity", "media.speed"],
} as const;
type Family = keyof typeof families;
type SpecialFamily = "Color" | "Position" | "Beam" | "Shapers" | "Control";
const labels: Record<string, string> = {
  intensity: "Dimmer",
  shutter: "Shutter",
  strobe: "Strobe",
  master: "Master",
  pan: "Pan",
  tilt: "Tilt",
  gobo: "Gobo 1",
  "gobo.2": "Gobo 2",
  "gobo.rotation": "Gobo rotation",
  prism: "Prism 1",
  "prism.2": "Prism 2",
  iris: "Iris",
  focus: "Focus",
  zoom: "Zoom",
  frost: "Frost",
  edge: "Edge",
};
const specialFamilies = new Set<SpecialFamily>([
  "Color",
  "Position",
  "Beam",
  "Shapers",
  "Control",
]);

export function ParameterControls() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [family, setFamily] = useState<Family>("Intensity");
  const [alignIndex, setAlignIndex] = useState(0);
  const [dynamicsMode, setDynamicsMode] = useState(false);
  const [visualization, setVisualization] =
    useState<VisualizationSnapshot | null>(null);
  useEffect(() => {
    if (!server.selectedFixtures.length) {
      setVisualization(null);
      return;
    }
    let cancelled = false;
    const refresh = () =>
      void server
        .readVisualization()
        .then((value) => {
          if (!cancelled) setVisualization(value);
        })
        .catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 400);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [server.selectedFixtures, server.readVisualization]);
  const supported = useMemo(() => {
    const result = new Set<string>();
    for (const fixture of server.patch?.fixtures ?? [])
      if (
        server.selectedFixtures.includes(fixture.fixture_id) ||
        fixture.logical_heads.some((head) =>
          server.selectedFixtures.includes(head.fixture_id),
        )
      )
        for (const head of fixture.definition.heads ?? [])
          for (const parameter of head.parameters)
            result.add(parameter.attribute);
    return result;
  }, [server.patch, server.selectedFixtures]);
  const values = useMemo(() => {
    const result = new Map<string, number>();
    for (const entry of visualization?.values ?? [])
      if (
        server.selectedFixtures.includes(entry.fixture_id) &&
        entry.value.kind === "normalized" &&
        !result.has(entry.attribute)
      )
        result.set(entry.attribute, entry.value.value);
    return result;
  }, [visualization, server.selectedFixtures]);
  const applyParameter = async (attribute: string, level: number) => {
    if (server.selectedGroupId) {
      await (state.preload === "blind"
        ? server.setPreloadGroupValue(attribute, level)
        : server.setGroupValue(attribute, level));
      return;
    }
    await Promise.all(
      server.selectedFixtures.map((fixtureId) =>
        server.setProgrammer(fixtureId, attribute, level),
      ),
    );
  };
  const attributes = families[family].filter((attribute) =>
    supported.has(attribute),
  );
  const encoderSlots = Array.from({ length: 6 }, (_, index) => attributes[index] ?? null);
  const lampMacros = useMemo(() => {
    const macros: Array<{ label: string; attribute: string; value: number }> = [];
    for (const fixture of server.patch?.fixtures ?? []) {
      if (!server.selectedFixtures.includes(fixture.fixture_id) && !fixture.logical_heads.some((head) => server.selectedFixtures.includes(head.fixture_id))) continue;
      for (const head of fixture.definition.heads ?? []) for (const parameter of head.parameters) {
        for (const capability of parameter.capabilities ?? []) {
          const name = capability.name.toLowerCase();
          const label = name.includes("lamp on") ? "Lamp on" : name.includes("lamp off") ? "Lamp off" : null;
          if (label && !macros.some((macro) => macro.label === label)) macros.push({ label, attribute: parameter.attribute, value: ((capability.dmx_from + capability.dmx_to) / 2) / 255 });
        }
      }
    }
    return macros;
  }, [server.patch, server.selectedFixtures]);
  if (state.stageMode !== "select" && (state.builtIn === "stage" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "stage"))) return <StageCommandControls />;
  return (
    <div className="parameter-controls">
      <div className="family-tabs">
        {(Object.keys(families) as Family[]).map((name) => (
            <button
              onClick={() => setFamily(name)}
              className={family === name ? "active" : ""}
              key={name}
            >
              {name}
            </button>
          ))}
        <span className="family-spacer" />
        {family === "Position" && <button className="align-cycle" onClick={() => {
          const modes = ["left", "right", "center", "out"] as const;
          const mode = modes[alignIndex];
          void server.alignSelection("pan", mode);
          setAlignIndex((alignIndex + 1) % modes.length);
        }}>Align {(["Left", "Right", "Center", "Out"] as const)[alignIndex]}</button>}
        {specialFamilies.has(family as SpecialFamily) && (
            <button
              className="special-dialogs"
              onClick={() => dispatch({ type: "OPEN_SPECIAL_DIALOG", family: family as SpecialFamily })}
            >
              ◇ Special Dialog
            </button>
        )}
        <button onClick={() => setDynamicsMode(!dynamicsMode)} className={`dynamics-family ${dynamicsMode ? "active" : ""}`}>Dynamics</button>
      </div>
      <div className="parameter-surfaces">
        {!server.selectedFixtures.length && !server.selectedGroupId ? (
          <div className="parameter-empty">
            <b>No fixtures selected</b>
            <small>
              Select fixtures to inspect or edit their real parameters.
            </small>
          </div>
        ) : (
          <>{encoderSlots.map((attribute, index) => {
            if (!attribute) return <div className="parameter-placeholder" aria-label={`Encoder ${index + 1} unassigned`} key={`empty-${index}`}><span>Encoder {index + 1}</span><small>Unassigned</small></div>;
            const value = values.get(attribute) ?? 0;
            return (
              <VerticalTouchFader
                key={attribute}
                label={labels[attribute] ?? attribute.replaceAll(".", " ")}
                value={value * 100}
                display={`${Math.round(value * 100)}%`}
                accentColor={attribute === "color.red" ? "#ff3d45" : attribute === "color.green" ? "#35d568" : attribute === "color.blue" ? "#378eff" : attribute === "color.white" ? "#ffffff" : attribute === "color.amber" ? "#ffb30f" : attribute === "color.uv" ? "#9a55ff" : undefined}
                mode={dynamicsMode ? "Dynamics" : undefined}
                onChange={(next) => void applyParameter(attribute, next / 100)}
              />
            );
          })}</>
        )}
      </div>
    </div>
  );
}
