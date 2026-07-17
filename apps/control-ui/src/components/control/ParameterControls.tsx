import { useEffect, useMemo, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import type { VisualizationSnapshot } from "../../api/types";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { StageCommandControls } from "./StageCommandControls";
import { Button } from "../common";

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
const alignModes = ["out", "center", "left", "right"] as const;
type AlignMode = typeof alignModes[number];
const compactFamilyLabels: Record<Family, string> = {
  Intensity: "Int",
  Color: "Col",
  Position: "Pos",
  Beam: "Beam",
  Shapers: "Shapr",
  Focus: "Focus",
  Control: "Ctrl",
  Media: "Media",
};
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

function FamilyLabel({ full, compact }: { full: string; compact: string }) {
  return <>
    <span className="family-label-full" aria-hidden="true">{full}</span>
    <span className="family-label-compact" aria-hidden="true">{compact}</span>
  </>;
}

function normalizedProgrammerTarget(value: unknown): number | undefined {
  if (typeof value === "number") return value;
  if (!value || typeof value !== "object") return undefined;
  const record = value as Record<string, unknown>;
  if (record.kind === "normalized" && typeof record.value === "number") {
    return record.value;
  }
  return record.value === value ? undefined : normalizedProgrammerTarget(record.value);
}

export function ParameterControls() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const ownProgrammer = server.bootstrap?.active_programmers.find(
    (programmer) => programmer.session_id === server.session?.session_id,
  );
  const programmerValues = (ownProgrammer?.values ?? []) as Array<{
    fixture_id: string;
    attribute: string;
    value: unknown;
  }>;
  const groupProgrammerValues = (ownProgrammer?.group_values ?? {}) as Record<
    string,
    Record<string, unknown>
  >;
  const [family, setFamily] = useState<Family>("Intensity");
  const [alignMode, setAlignMode] = useState<AlignMode | null>(null);
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
    if (server.selectedGroupId) {
      // An empty stored Group is still a valid live programming target. Keep the portable
      // intensity control available before it has members, and retain any attributes already
      // carried by the Group so they can be inspected or released.
      result.add("intensity");
      const group = server.groups.find((candidate) => candidate.id === server.selectedGroupId);
      for (const attribute of Object.keys(group?.body.programming ?? {})) result.add(attribute);
    }
    return result;
  }, [server.patch, server.selectedFixtures, server.selectedGroupId, server.groups]);
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
  const programmerTarget = (attribute: string): number | undefined => {
    if (server.selectedGroupId) {
      return normalizedProgrammerTarget(
        groupProgrammerValues[server.selectedGroupId]?.[attribute],
      );
    }
    for (const fixtureId of server.selectedFixtures) {
      const entry = programmerValues.find(
        (candidate) =>
          candidate.fixture_id === fixtureId && candidate.attribute === attribute,
      );
      const target = normalizedProgrammerTarget(entry?.value);
      if (target != null) return target;
    }
    return undefined;
  };
  const applyParameter = async (attribute: string, level: number) => {
    if (server.selectedGroupId) {
      // The server owns the capture-domain decision. Sending the normal programmer action keeps
      // this control live when programmer capture is disabled and blind when it is enabled.
      await server.setGroupValue(attribute, level);
      return;
    }
    await Promise.all(
      server.selectedFixtures.map((fixtureId) =>
        server.setProgrammer(fixtureId, attribute, level),
      ),
    );
  };
  const releaseParameter = async (attribute: string) => {
    if (server.selectedGroupId) {
      await server.releaseGroupValue(attribute);
      return;
    }
    const fixtureValues = new Set(
      programmerValues
        .filter((entry) => entry.attribute === attribute)
        .map((entry) => entry.fixture_id),
    );
    await Promise.all(
      server.selectedFixtures
        .filter((fixtureId) => fixtureValues.has(fixtureId))
        .map((fixtureId) => server.releaseProgrammer(fixtureId, attribute)),
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
            <Button
              onClick={() => setFamily(name)}
              className={`attribute-family ${family === name ? "active" : ""}`}
              key={name}
              aria-label={name}
            >
              <FamilyLabel full={name} compact={compactFamilyLabels[name]} />
            </Button>
          ))}
        <span className="family-spacer" />
        {family === "Position" && <Button aria-label={`Align ${alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}`} className={`align-cycle ${alignMode ? "align-active" : "align-off"}`} onClick={(event) => {
          if (event.shiftKey || state.shiftArmed) {
            setAlignMode(null);
            if (state.shiftArmed) dispatch({ type: "SET_SHIFT_ARMED", value: false });
            return;
          }
          const next = alignModes[(alignMode == null ? 0 : alignModes.indexOf(alignMode) + 1) % alignModes.length];
          void server.alignSelection("pan", next);
          setAlignMode(next);
        }}><span className="align-label-full"><span>Align</span><span>{alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}</span></span><span className="align-label-compact"><span>Align</span><span>{alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}</span></span></Button>}
        {specialFamilies.has(family as SpecialFamily) && (
            <Button
              className="special-dialogs"
              aria-label="Special Dialog"
              onClick={() => dispatch({ type: "OPEN_SPECIAL_DIALOG", family: family as SpecialFamily })}
            >
              <span className="special-dialog-label-full"><span>Special</span><span>Dialog</span></span>
              <span className="special-dialog-label-compact">Spcl</span>
            </Button>
        )}
        <Button aria-label="Dynamics" onClick={() => setDynamicsMode(!dynamicsMode)} className={`dynamics-family ${dynamicsMode ? "active" : ""}`}><FamilyLabel full="Dynamics" compact="Dyn" /></Button>
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
            if (!attribute) return <div className="parameter-placeholder" aria-label={`Encoder ${index + 1} unassigned`} key={`empty-${index}`}><span>Enc {index + 1}</span><small>Unassigned</small></div>;
            // Encoders show the operator's target immediately. Fixture/Stage views continue to
            // read the resolved visualization so a configured Programmer Fade stays visible.
            const value = programmerTarget(attribute) ?? values.get(attribute) ?? 0;
            const hasScopedValue = server.selectedGroupId
              ? Boolean(ownProgrammer?.group_values?.[server.selectedGroupId]?.[attribute])
              : programmerValues.some(
                  (entry) => entry.attribute === attribute && server.selectedFixtures.includes(entry.fixture_id),
                );
            return (
              <VerticalTouchFader
                key={attribute}
                label={`Enc ${index + 1} · ${labels[attribute] ?? attribute.replaceAll(".", " ")}`}
                value={value * 100}
                display={`${Math.round(value * 100)}%`}
                accentColor={attribute === "color.red" ? "#ff3d45" : attribute === "color.green" ? "#35d568" : attribute === "color.blue" ? "#378eff" : attribute === "color.white" ? "#ffffff" : attribute === "color.amber" ? "#ffb30f" : attribute === "color.uv" ? "#9a55ff" : undefined}
                mode={dynamicsMode ? "Dynamics" : undefined}
                directInput
                actions={hasScopedValue ? [{ id: "release", label: "Release", "aria-label": `Release ${labels[attribute] ?? attribute}` , onClick: () => void releaseParameter(attribute) }] : []}
                onChange={(next) => void applyParameter(attribute, next / 100)}
              />
            );
          })}</>
        )}
      </div>
    </div>
  );
}
