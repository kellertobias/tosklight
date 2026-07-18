import { useEffect, useMemo, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import type {
  AttributeValue,
  ControlActionKind,
  PatchedFixture,
  VisualizationSnapshot,
} from "../../api/types";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { StageCommandControls } from "./StageCommandControls";
import { Button } from "../common";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

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

function discreteProgrammerTarget(value: unknown): string | undefined {
  if (!value || typeof value !== "object") return undefined;
  const record = value as Record<string, unknown>;
  if (record.kind === "discrete" && typeof record.value === "string") return record.value;
  return record.value === value ? undefined : discreteProgrammerTarget(record.value);
}

function formatNormalizedValue(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function formatNormalizedRange(values: number[]): string | undefined {
  if (!values.length) return undefined;
  const rounded = values.map((value) => Math.round(value * 100));
  const minimum = Math.min(...rounded);
  const maximum = Math.max(...rounded);
  return minimum === maximum ? `${minimum}%` : `${minimum}%...${maximum}%`;
}

function formatDiscreteValues(values: string[]): string | undefined {
  if (!values.length) return undefined;
  const unique = [...new Set(values)];
  return unique.length === 1 ? unique[0] : "Mixed";
}

interface DirectValueAssignment {
  fixtureId: string;
  attribute: string;
}

export interface DirectValueChoice {
  key: string;
  label: string;
  semanticId: string;
  kind: "fixed" | "indexed";
  assignments: DirectValueAssignment[];
}

export interface DirectControlChoice {
  key: string;
  actionId: string;
  label: string;
  kind: ControlActionKind;
  durationMillis: number | null;
  fixtureIds: string[];
}

function profileHeadOwner(
  fixture: PatchedFixture,
  headId: string,
): string | null {
  const profile = fixture.definition.profile_snapshot;
  const mode = profile?.modes.find((candidate) => candidate.id === fixture.definition.mode_id);
  const headIndex = mode?.heads.findIndex((head) => head.id === headId) ?? -1;
  if (!mode || headIndex < 0) return null;
  if (mode.heads[headIndex].master_shared) return fixture.fixture_id;
  return (
    fixture.logical_heads.find((head) => head.head_index === headIndex) ??
    fixture.logical_heads.find((head) => head.head_index === headIndex + 1)
  )?.fixture_id ?? null;
}

export function directProgrammerChoices(
  fixtures: PatchedFixture[],
  selectedFixtures: string[],
): {
  values: DirectValueChoice[];
  actions: DirectControlChoice[];
  fixtureIds: string[];
} {
  const selected = new Set(selectedFixtures);
  const values = new Map<string, DirectValueChoice>();
  const actions = new Map<string, DirectControlChoice>();
  const fixtureIds = new Set<string>();
  for (const fixture of fixtures) {
    const physicalSelected = selected.has(fixture.fixture_id);
    const logicalSelected = fixture.logical_heads.some((head) => selected.has(head.fixture_id));
    if (!physicalSelected && !logicalSelected) continue;
    const profile = fixture.definition.profile_snapshot;
    const mode = profile?.modes.find((candidate) => candidate.id === fixture.definition.mode_id);
    if (!profile || !mode) continue;
    for (const channel of mode.channels) {
      const owner = profileHeadOwner(fixture, channel.head_id);
      if (!owner || (!physicalSelected && !selected.has(owner))) continue;
      for (const fn of channel.functions) {
        if (fn.behavior.type !== "fixed" && fn.behavior.type !== "indexed") continue;
        const key = `${fn.behavior.type}:${fn.behavior.semantic_id}`;
        const choice = values.get(key) ?? {
          key,
          label: fn.behavior.label,
          semanticId: fn.behavior.semantic_id,
          kind: fn.behavior.type,
          assignments: [],
        };
        if (fn.behavior.label.localeCompare(choice.label) < 0) choice.label = fn.behavior.label;
        if (!choice.assignments.some(
          (assignment) => assignment.fixtureId === owner && assignment.attribute === fn.attribute,
        )) {
          choice.assignments.push({ fixtureId: owner, attribute: fn.attribute });
        }
        values.set(key, choice);
        fixtureIds.add(fixture.fixture_id);
      }
    }
    for (const action of mode.control_actions) {
      const key = `${profile.id}:${mode.id}:${action.id}`;
      const choice = actions.get(key) ?? {
        key,
        actionId: action.id,
        label: action.name,
        kind: action.kind,
        durationMillis: action.duration_millis,
        fixtureIds: [],
      };
      if (!choice.fixtureIds.includes(fixture.fixture_id)) choice.fixtureIds.push(fixture.fixture_id);
      actions.set(key, choice);
    }
  }
  return {
    values: [...values.values()].sort((left, right) => left.label.localeCompare(right.label)),
    actions: [...actions.values()].sort((left, right) => left.label.localeCompare(right.label)),
    fixtureIds: [...fixtureIds],
  };
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
  const [directMode, setDirectMode] = useState(false);
  const [latchedActions, setLatchedActions] = useState<Record<string, boolean>>({});
  const [generationStatus, setGenerationStatus] = useState<string | null>(null);
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
  const directChoices = useMemo(
    () => directProgrammerChoices(server.patch?.fixtures ?? [], server.selectedFixtures),
    [server.patch, server.selectedFixtures],
  );
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
  const normalizedValuesByFixture = useMemo(() => {
    const result = new Map<string, Map<string, number>>();
    for (const entry of visualization?.values ?? []) {
      if (!server.selectedFixtures.includes(entry.fixture_id) || entry.value.kind !== "normalized") continue;
      const fixtureValues = result.get(entry.fixture_id) ?? new Map<string, number>();
      fixtureValues.set(entry.attribute, entry.value.value);
      result.set(entry.fixture_id, fixtureValues);
    }
    return result;
  }, [visualization, server.selectedFixtures]);
  const discreteValues = useMemo(() => {
    const result = new Map<string, string>();
    for (const entry of visualization?.values ?? [])
      if (server.selectedFixtures.includes(entry.fixture_id) && entry.value.kind === "discrete" && !result.has(entry.attribute))
        result.set(entry.attribute, entry.value.value);
    return result;
  }, [visualization, server.selectedFixtures]);
  const discreteValuesByFixture = useMemo(() => {
    const result = new Map<string, Map<string, string>>();
    for (const entry of visualization?.values ?? []) {
      if (!server.selectedFixtures.includes(entry.fixture_id) || entry.value.kind !== "discrete") continue;
      const fixtureValues = result.get(entry.fixture_id) ?? new Map<string, string>();
      fixtureValues.set(entry.attribute, entry.value.value);
      result.set(entry.fixture_id, fixtureValues);
    }
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
  const programmerDiscreteTarget = (attribute: string): string | undefined => {
    if (server.selectedGroupId) return discreteProgrammerTarget(groupProgrammerValues[server.selectedGroupId]?.[attribute]);
    for (const fixtureId of server.selectedFixtures) {
      const entry = programmerValues.find((candidate) => candidate.fixture_id === fixtureId && candidate.attribute === attribute);
      const target = discreteProgrammerTarget(entry?.value);
      if (target != null) return target;
    }
    return undefined;
  };
  const encoderNormalizedDisplay = (attribute: string): string | undefined => {
    if (server.selectedGroupId) {
      const target = programmerTarget(attribute);
      return target == null ? undefined : formatNormalizedValue(target);
    }
    const targets = server.selectedFixtures.flatMap((fixtureId) => {
      const entry = programmerValues.find((candidate) => candidate.fixture_id === fixtureId && candidate.attribute === attribute);
      const target = normalizedProgrammerTarget(entry?.value);
      const resolved = normalizedValuesByFixture.get(fixtureId)?.get(attribute);
      const value = target ?? resolved;
      return value == null ? [] : [value];
    });
    return formatNormalizedRange(targets);
  };
  const encoderDiscreteDisplay = (attribute: string): string | undefined => {
    if (server.selectedGroupId) return programmerDiscreteTarget(attribute);
    const targets = server.selectedFixtures.flatMap((fixtureId) => {
      const entry = programmerValues.find((candidate) => candidate.fixture_id === fixtureId && candidate.attribute === attribute);
      const target = discreteProgrammerTarget(entry?.value);
      const resolved = discreteValuesByFixture.get(fixtureId)?.get(attribute);
      const value = target ?? resolved;
      return value == null ? [] : [value];
    });
    return formatDiscreteValues(targets);
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
  const applyParameterRange = async (attribute: string, percentages: number[]) => {
    const points = percentages.map((value) => Math.max(0, Math.min(100, value)) / 100);
    if (server.selectedGroupId) {
      await server.setGroupValue(attribute, { kind: "spread", value: points });
      return;
    }
    const count = server.selectedFixtures.length;
    const valueAt = (index: number) => {
      if (points.length === 1 || count <= 1) return points[0] ?? 0;
      const position = index * (points.length - 1) / (count - 1);
      const left = Math.floor(position);
      const right = Math.ceil(position);
      return points[left] + (points[right] - points[left]) * (position - left);
    };
    await server.setProgrammerMany(server.selectedFixtures.map((fixtureId, index) => ({
      fixtureId,
      attribute,
      value: valueAt(index),
    })));
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
  const applyDirectValue = async (choice: DirectValueChoice) => {
    const value: AttributeValue = { kind: "discrete", value: choice.semanticId };
    await Promise.all(
      choice.assignments.map((assignment) =>
        server.setProgrammerValue(assignment.fixtureId, assignment.attribute, value),
      ),
    );
  };
  const applyControlAction = async (choice: DirectControlChoice, active: boolean) => {
    await Promise.all(
      choice.fixtureIds.map((fixtureId) =>
        server.controlFixtureAction(fixtureId, choice.actionId, active),
      ),
    );
  };
  const generateDirectPresets = async () => {
    setGenerationStatus("Generating portable presets…");
    const result = await server.generateFixturePresets(directChoices.fixtureIds);
    setGenerationStatus(
      result
        ? `Created ${result.created.length} portable preset${result.created.length === 1 ? "" : "s"}`
        : "Preset generation failed",
    );
  };
  const directChoiceActive = (choice: DirectValueChoice) => choice.assignments.some(
    (assignment) => programmerValues.some(
      (entry) => entry.fixture_id === assignment.fixtureId
        && entry.attribute === assignment.attribute
        && discreteProgrammerTarget(entry.value) === choice.semanticId,
    ),
  );
  const attributes = families[family].filter((attribute) =>
    supported.has(attribute),
  );
  const encoderSlots = Array.from({ length: 6 }, (_, index) => attributes[index] ?? null);
  const hardwareConnected = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  useEffect(() => {
    if (!hardwareConnected || directMode) return;
    const handleEncoder = (event: Event) => {
      const { control, value } = (event as CustomEvent<{ control: string; value?: string }>).detail;
      const slot = Number(control.split("/")[1]) - 1;
      const attribute = encoderSlots[slot];
      if (!attribute || !["up", "down", "left", "right"].includes(value ?? "")) return;
      if (programmerDiscreteTarget(attribute) ?? discreteValues.get(attribute)) return;
      const current = programmerTarget(attribute) ?? values.get(attribute) ?? 0;
      const delta = value === "up" ? .01 : value === "down" ? -.01 : value === "right" ? .1 : -.1;
      void applyParameter(attribute, Math.max(0, Math.min(1, current + delta)));
    };
    window.addEventListener("light:encoder-action", handleEncoder);
    return () => window.removeEventListener("light:encoder-action", handleEncoder);
  }, [hardwareConnected, directMode, encoderSlots.join("|"), programmerValues, groupProgrammerValues, values, discreteValues, server.selectedFixtures, server.selectedGroupId]);
  if (state.stageMode !== "select" && (state.builtIn === "stage" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "stage"))) return <StageCommandControls />;
  return (
    <div className="parameter-controls">
      <div className="family-tabs">
        {(Object.keys(families) as Family[]).map((name) => (
            <Button
              onClick={() => {
                setFamily(name);
                setDirectMode(false);
              }}
              className={`attribute-family ${!directMode && family === name ? "active" : ""}`}
              key={name}
              aria-label={name}
            >
              <FamilyLabel full={name} compact={compactFamilyLabels[name]} />
            </Button>
          ))}
        <Button
          aria-label="Direct values and actions"
          className={`attribute-family direct-family ${directMode ? "active" : ""}`}
          onClick={() => setDirectMode(true)}
        >
          <FamilyLabel full="Direct" compact="Dir" />
        </Button>
        <span className="family-spacer" />
        {!directMode && family === "Position" && <Button aria-label={`Align ${alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}`} className={`align-cycle ${alignMode ? "align-active" : "align-off"}`} onClick={(event) => {
          if (event.shiftKey || state.shiftArmed) {
            setAlignMode(null);
            if (state.shiftArmed) dispatch({ type: "SET_SHIFT_ARMED", value: false });
            return;
          }
          const next = alignModes[(alignMode == null ? 0 : alignModes.indexOf(alignMode) + 1) % alignModes.length];
          void server.alignSelection("pan", next);
          setAlignMode(next);
        }}><span className="align-label-full"><span>Align</span><span>{alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}</span></span><span className="align-label-compact"><span>Align</span><span>{alignMode ? alignMode[0].toUpperCase() + alignMode.slice(1) : "Off"}</span></span></Button>}
        {!directMode && specialFamilies.has(family as SpecialFamily) && (
            <Button
              className="special-dialogs"
              aria-label="Special Dialog"
              onClick={() => dispatch({ type: "OPEN_SPECIAL_DIALOG", family: family as SpecialFamily })}
            >
              <span className="special-dialog-label-full"><span>Special</span><span>Dialog</span></span>
              <span className="special-dialog-label-compact">Spcl</span>
            </Button>
        )}
        <Button aria-label="Dynamics" onClick={() => { setDirectMode(false); setDynamicsMode(!dynamicsMode); }} className={`dynamics-family ${dynamicsMode ? "active" : ""}`}><FamilyLabel full="Dynamics" compact="Dyn" /></Button>
      </div>
      <div className="parameter-surfaces">
        {directMode && hardwareConnected ? (
          <>{Array.from({ length: 6 }, (_, index) => <HardwareEncoderDisplay key={index} slot={index + 1} />)}</>
        ) : directMode ? (
          <section className="direct-programmer-picker" aria-label="Direct programmer values and actions">
            <header>
              <div>
                <b>Fixed, indexed, and control values</b>
                <small>Semantic values stay portable across fixture-profile DMX ranges.</small>
              </div>
              <Button
                disabled={!directChoices.values.length || Boolean(server.selectedGroupId)}
                onClick={() => void generateDirectPresets()}
              >
                Generate portable presets
              </Button>
            </header>
            {server.selectedGroupId && (
              <p role="note">Select concrete fixtures to use typed direct values or generate presets.</p>
            )}
            {!server.selectedGroupId && !directChoices.values.length && !directChoices.actions.length ? (
              <div className="direct-programmer-empty">
                <b>No direct values configured</b>
                <small>The selected profile mode has no fixed/indexed functions or typed control actions.</small>
              </div>
            ) : (
              <div className="direct-programmer-columns">
                <section>
                  <h3>Fixed and indexed values</h3>
                  <div className="direct-value-grid">
                    {directChoices.values.map((choice) => (
                      <Button
                        key={choice.key}
                        disabled={Boolean(server.selectedGroupId)}
                        className={directChoiceActive(choice) ? "active" : ""}
                        aria-label={`${choice.label} ${choice.kind} value`}
                        onClick={() => void applyDirectValue(choice)}
                      >
                        <b>{choice.label}</b>
                        <small>{choice.kind} · {choice.assignments[0]?.attribute}</small>
                      </Button>
                    ))}
                  </div>
                </section>
                <section>
                  <h3>Control actions</h3>
                  <div className="direct-value-grid">
                    {directChoices.actions.map((choice) => {
                      const active = Boolean(latchedActions[choice.key]);
                      const shared = {
                        key: choice.key,
                        disabled: Boolean(server.selectedGroupId),
                        className: active ? "active" : "",
                        "aria-label": `${choice.label} ${choice.kind} control action`,
                      };
                      const content = <><b>{choice.label}</b><small>{choice.kind.replaceAll("_", " ")}{choice.durationMillis != null ? ` · ${choice.durationMillis} ms` : ""}</small></>;
                      if (choice.kind === "momentary") {
                        return <Button
                          {...shared}
                          onPointerDown={(event) => {
                            event.currentTarget.setPointerCapture?.(event.pointerId);
                            void applyControlAction(choice, true);
                          }}
                          onPointerUp={(event) => {
                            if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
                            void applyControlAction(choice, false);
                          }}
                          onPointerCancel={() => void applyControlAction(choice, false)}
                          onKeyDown={(event) => {
                            if (!event.repeat && (event.key === "Enter" || event.key === " ")) {
                              event.preventDefault();
                              void applyControlAction(choice, true);
                            }
                          }}
                          onKeyUp={(event) => {
                            if (event.key === "Enter" || event.key === " ") {
                              event.preventDefault();
                              void applyControlAction(choice, false);
                            }
                          }}
                        >{content}</Button>;
                      }
                      return <Button
                        {...shared}
                        onClick={() => {
                          const next = choice.kind === "latched" ? !active : true;
                          if (choice.kind === "latched") {
                            setLatchedActions((current) => ({ ...current, [choice.key]: next }));
                          }
                          void applyControlAction(choice, next);
                        }}
                      >{content}</Button>;
                    })}
                  </div>
                </section>
              </div>
            )}
            {generationStatus && <footer role="status">{generationStatus}</footer>}
          </section>
        ) : !server.selectedFixtures.length && !server.selectedGroupId ? (
          <div className="parameter-empty">
            <b>No fixtures selected</b>
            <small>
              Select fixtures to inspect or edit their real parameters.
            </small>
          </div>
        ) : (
          <>{encoderSlots.map((attribute, index) => {
            if (!attribute) return hardwareConnected
              ? <HardwareEncoderDisplay key={`empty-${index}`} slot={index + 1} />
              : <div className="parameter-placeholder" aria-label={`Encoder ${index + 1} unassigned`} key={`empty-${index}`}><span>Enc {index + 1}</span><small>Unassigned</small></div>;
            // Encoders show the operator's target immediately. Fixture/Stage views continue to
            // read the resolved visualization so a configured Programmer Fade stays visible.
            const value = programmerTarget(attribute) ?? values.get(attribute) ?? 0;
            const discreteValue = encoderDiscreteDisplay(attribute);
            const normalizedDisplay = encoderNormalizedDisplay(attribute) ?? formatNormalizedValue(value);
            const hasScopedValue = server.selectedGroupId
              ? Boolean(ownProgrammer?.group_values?.[server.selectedGroupId]?.[attribute])
              : programmerValues.some(
                  (entry) => entry.attribute === attribute && server.selectedFixtures.includes(entry.fixture_id),
                );
            return hardwareConnected ? <HardwareEncoderDisplay
              key={attribute}
              slot={index + 1}
              target={{ label: labels[attribute] ?? attribute.replaceAll(".", " "), value: discreteValue ?? normalizedDisplay }}
              editValue={discreteValue ? undefined : value * 100}
              onEdit={discreteValue ? undefined : (next) => void applyParameter(attribute, Math.max(0, Math.min(100, next)) / 100)}
              onEditRange={discreteValue ? undefined : (points) => void applyParameterRange(attribute, points)}
              onRelease={hasScopedValue ? () => void releaseParameter(attribute) : undefined}
            /> : (
              <VerticalTouchFader
                key={attribute}
                label={`Enc ${index + 1} · ${labels[attribute] ?? attribute.replaceAll(".", " ")}`}
                value={value * 100}
                display={formatNormalizedValue(value)}
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
