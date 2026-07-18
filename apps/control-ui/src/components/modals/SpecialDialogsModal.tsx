import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "../control/VerticalTouchFader";
import {
  moveLampPositions,
  resolveLampPositions,
  returnHomeAssignments,
} from "./specialPosition";
import {
  colorProgrammerAssignments,
  hsvToRgb,
  interpolatePickerRange,
  type PickerColor,
} from "./specialColor";
import { Button } from "../common";
import type { ControlActionKind, ControlActionSemantic, PatchedFixture } from "../../api/types";

export interface CompatibleFixtureControlAction {
  fixtureId: string;
  actionId: string;
  kind: ControlActionKind;
}

function inferredControlSemantic(name: string): ControlActionSemantic {
  const normalized = name.trim().toLowerCase().replaceAll(/[^a-z0-9]+/g, " ");
  if (/^(lamp on|strike|ignite)( lamp)?$/.test(normalized)) return "lamp_on";
  if (/^lamp off$/.test(normalized)) return "lamp_off";
  if (/^reset$/.test(normalized)) return "reset";
  if (/^fan auto$/.test(normalized)) return "fan_auto";
  if (/^fan low$/.test(normalized)) return "fan_low";
  if (/^fan high$/.test(normalized)) return "fan_high";
  if (/^fan max(imum)?$/.test(normalized)) return "fan_max";
  return "custom";
}

export function compatibleSpecialDialogActions(
  fixtures: PatchedFixture[],
  semantic: ControlActionSemantic,
  selectedFixtureIds: string[] = [],
): CompatibleFixtureControlAction[] {
  const selected = new Set(selectedFixtureIds);
  return fixtures.flatMap((fixture) => {
    if (selected.size && !selected.has(fixture.fixture_id)
      && !fixture.logical_heads.some((head) => selected.has(head.fixture_id))) return [];
    const profile = fixture.definition.profile_snapshot;
    const mode = profile?.modes.find((candidate) => candidate.id === fixture.definition.mode_id);
    if (!mode) return [];
    return mode.control_actions
      .filter((action) => (
        action.semantic && action.semantic !== "custom"
          ? action.semantic
          : inferredControlSemantic(action.name)
      ) === semantic)
      .map((action) => ({ fixtureId: fixture.fixture_id, actionId: action.id, kind: action.kind }));
  });
}

export function SpecialDialogsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [pan, setPan] = useState(0.5),
    [tilt, setTilt] = useState(0.5);
  const [hue, setHue] = useState(0.52),
    [saturation, setSaturation] = useState(0.8),
    [brightness, setBrightness] = useState(0.85);
  const [colorRangePreview, setColorRangePreview] = useState<{
    start: PickerColor;
    end: PickerColor;
    active: boolean;
  } | null>(null);
  const [beamPage, setBeamPage] = useState(0),
    [dynamicSpeed, setDynamicSpeed] = useState(30);
  const trackball = useRef<HTMLDivElement>(null),
    colorSheet = useRef<HTMLDivElement>(null);
  const joystick = useRef({ x: 0, y: 0 });
  const colorRangeGesture = useRef<{
    pointerId: number;
    start: PickerColor;
  } | null>(null);
  const fixturePositions = useRef(new Map<string, { pan: number; tilt: number }>());
  const selectedFixtureKey = server.selectedFixtures.join("\u0000");
  const homeAssignments = useMemo(
    () =>
      returnHomeAssignments(
        server.selectedFixtures,
        server.patch?.fixtures ?? [],
      ),
    [server.patch, server.selectedFixtures],
  );
  const available = useMemo(() => {
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
  const family = state.specialDialogFamily;
  const close = () =>
    dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });
  const apply = async (attribute: string, value: number) => {
    const actions = server.selectedFixtures.map((fixtureId) => ({ fixtureId, attribute }));
    await Promise.all(actions.map((action) =>
      server.setProgrammer(action.fixtureId, action.attribute, value),
    ));
  };
  const fixtureControlActions = (semantic: ControlActionSemantic, allWhenEmpty = false) => {
    if (!server.selectedFixtures.length && !allWhenEmpty) return [];
    return compatibleSpecialDialogActions(
      server.patch?.fixtures ?? [],
      semantic,
      server.selectedFixtures,
    );
  };
  const applyFixtureControl = async (
    semantic: ControlActionSemantic,
    phase: "click" | "press" | "release",
    allWhenEmpty = false,
  ) => {
    const actions = fixtureControlActions(semantic, allWhenEmpty).filter((action) =>
      phase === "click" ? action.kind !== "momentary" : action.kind === "momentary",
    );
    if (!actions.length) return;
    await Promise.all(actions.map((action) =>
      server.controlFixtureAction(action.fixtureId, action.actionId, phase !== "release"),
    ));
  };
  const controlButtonProps = (semantic: ControlActionSemantic, allWhenEmpty = false) => ({
    onClick: () => void applyFixtureControl(semantic, "click", allWhenEmpty),
    onPointerDown: () => void applyFixtureControl(semantic, "press", allWhenEmpty),
    onPointerUp: () => void applyFixtureControl(semantic, "release", allWhenEmpty),
    onPointerCancel: () => void applyFixtureControl(semantic, "release", allWhenEmpty),
    onPointerLeave: () => void applyFixtureControl(semantic, "release", allWhenEmpty),
    onKeyDown: (event: KeyboardEvent) => {
      if (!event.repeat && (event.key === "Enter" || event.key === " "))
        void applyFixtureControl(semantic, "press", allWhenEmpty);
    },
    onKeyUp: (event: KeyboardEvent) => {
      if (event.key === "Enter" || event.key === " ")
        void applyFixtureControl(semantic, "release", allWhenEmpty);
    },
  });
  const applyColors = async (colors: PickerColor[]) => {
    const assignments = colorProgrammerAssignments(
      server.selectedFixtures,
      server.patch?.fixtures ?? [],
      colors,
    );
    if (assignments.length) await server.setProgrammerMany(assignments);
  };
  const point = (
    event: PointerEvent<HTMLDivElement>,
    ref: RefObject<HTMLDivElement | null>,
  ) => {
    const box = ref.current!.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(1, (event.clientX - box.left) / box.width)),
      y: Math.max(0, Math.min(1, (event.clientY - box.top) / box.height)),
    };
  };
  const movePosition = (event: PointerEvent<HTMLDivElement>) => {
    const next = point(event, trackball);
    joystick.current = { x: (next.x - 0.5) * 2, y: (next.y - 0.5) * 2 };
  };
  const releasePosition = () => {
    joystick.current = { x: 0, y: 0 };
  };
  const pickerColor = (event: PointerEvent<HTMLDivElement>): PickerColor => {
    const next = point(event, colorSheet);
    return { hue: next.x, saturation: 1 - next.y, brightness };
  };
  const moveColor = (event: PointerEvent<HTMLDivElement>) => {
    const next = pickerColor(event);
    setHue(next.hue);
    setSaturation(next.saturation);
    const gesture = colorRangeGesture.current;
    if (gesture?.pointerId === event.pointerId) {
      setColorRangePreview({ start: gesture.start, end: next, active: true });
      return;
    }
    void applyColors(server.selectedFixtures.map(() => next));
  };
  const startColor = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const start = pickerColor(event);
    if (event.shiftKey || state.shiftArmed) {
      colorRangeGesture.current = { pointerId: event.pointerId, start };
      setHue(start.hue);
      setSaturation(start.saturation);
      setColorRangePreview({ start, end: start, active: true });
      return;
    }
    moveColor(event);
  };
  const completeColor = (event: PointerEvent<HTMLDivElement>) => {
    const gesture = colorRangeGesture.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const end = pickerColor(event);
    colorRangeGesture.current = null;
    setHue(end.hue);
    setSaturation(end.saturation);
    setColorRangePreview({ start: gesture.start, end, active: false });
    void applyColors(
      interpolatePickerRange(
        server.selectedFixtures.length,
        gesture.start,
        end,
      ),
    );
  };
  const cancelColor = (event: PointerEvent<HTMLDivElement>) => {
    if (colorRangeGesture.current?.pointerId !== event.pointerId) return;
    colorRangeGesture.current = null;
    setColorRangePreview(null);
  };
  useEffect(() => {
    if (!state.specialDialogsOpen || state.specialDialogFamily !== "Position") return;
    let cancelled = false;
    void server.readVisualization().then((snapshot) => {
      if (cancelled) return;
      const origins = resolveLampPositions(
        server.selectedFixtures,
        server.patch?.fixtures ?? [],
        snapshot,
      );
      fixturePositions.current = origins;
      const values = [...origins.values()];
      if (values.length) {
        setPan(values.reduce((sum, value) => sum + value.pan, 0) / values.length);
        setTilt(values.reduce((sum, value) => sum + value.tilt, 0) / values.length);
      }
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [state.specialDialogsOpen, state.specialDialogFamily, selectedFixtureKey]);
  useEffect(() => {
    if (!state.specialDialogsOpen || state.specialDialogFamily !== "Position") return;
    const timer = window.setInterval(() => {
      const vector = joystick.current;
      const magnitude = Math.min(1, Math.hypot(vector.x, vector.y));
      if (magnitude < 0.04) return;
      const speed = 0.002 + magnitude * magnitude * 0.028;
      const positions = fixturePositions.current;
      if (!positions.size) return;
      const updates: Promise<void>[] = [];
      moveLampPositions(positions, vector.x, vector.y, speed);
      for (const [fixture, position] of positions) {
        updates.push(server.setProgrammer(fixture, "pan", position.pan));
        updates.push(server.setProgrammer(fixture, "tilt", position.tilt));
      }
      void Promise.all(updates);
      const values = [...positions.values()];
      setPan(values.reduce((sum, value) => sum + value.pan, 0) / values.length);
      setTilt(values.reduce((sum, value) => sum + value.tilt, 0) / values.length);
    }, 32);
    return () => window.clearInterval(timer);
  }, [state.specialDialogsOpen, state.specialDialogFamily, selectedFixtureKey]);
  if (!state.specialDialogsOpen) return null;
  const beamAttributes = [...available].filter((attribute) =>
    family === "Shapers"
      ? attribute.startsWith("shaper.")
      : /^(gobo|prism|iris)/.test(attribute),
  );
  const pageAttributes = beamAttributes.slice(beamPage * 4, beamPage * 4 + 4);
  const color = hsvToRgb({ hue, saturation, brightness });
  const swatch = `rgb(${color.map((channel) => Math.round(channel * 255)).join(",")})`;
  return (
    <div
      className="modal-backdrop"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <section className={`modal-card special-dialog-card ${family === "Position" ? "position-special-dialog" : ""}`}>
        <Button className="modal-close" onClick={close}>
          ×
        </Button>
        <h2>{family} · Special Dialog</h2>
        <p>{server.selectedFixtures.length} fixtures selected</p>
        <div className="special-dialog-content">
          {family === "Position" && (
            <div className="position-trackball-layout">
              <div
                ref={trackball}
                className="position-trackball"
                onPointerDown={(event) => {
                  event.currentTarget.setPointerCapture(event.pointerId);
                  movePosition(event);
                }}
                onPointerMove={(event) => {
                  if (event.currentTarget.hasPointerCapture(event.pointerId))
                    movePosition(event);
                }}
                onPointerUp={releasePosition}
                onPointerCancel={releasePosition}
                onLostPointerCapture={releasePosition}
              >
                <i className="joystick-handle" style={{ left: `${50 + joystick.current.x * 38}%`, top: `${50 + joystick.current.y * 38}%` }} />
              </div>
              <span className="position-trackball-readout">Relative move<br/><b>Avg Pan {Math.round(pan * 100)}%</b><b>Avg Tilt {Math.round(tilt * 100)}%</b>
                <Button
                  disabled={homeAssignments.length === 0}
                  onClick={async () => {
                    if (!(await server.setProgrammerMany(homeAssignments))) return;
                    const positions = new Map(fixturePositions.current);
                    for (const assignment of homeAssignments) {
                      const position = positions.get(assignment.fixtureId);
                      if (position) position[assignment.attribute] = assignment.value;
                    }
                    fixturePositions.current = positions;
                    const values = [...positions.values()];
                    if (values.length) {
                      setPan(values.reduce((sum, value) => sum + value.pan, 0) / values.length);
                      setTilt(values.reduce((sum, value) => sum + value.tilt, 0) / values.length);
                    }
                  }}
                >
                  Return Home
                </Button>
              </span>
            </div>
          )}
          {family === "Color" && (
            <div className="graphical-color-picker">
              <div
                ref={colorSheet}
                className="color-sheet"
                data-range-shift={state.shiftArmed ? "armed" : "idle"}
                style={{ backgroundColor: `hsl(${hue * 360} 100% 50%)` }}
                onPointerDown={startColor}
                onPointerMove={(event) => {
                  if (event.currentTarget.hasPointerCapture(event.pointerId))
                    moveColor(event);
                }}
                onPointerUp={completeColor}
                onPointerCancel={cancelColor}
                onLostPointerCapture={cancelColor}
              >
                {colorRangePreview && (
                  <svg
                    className="color-range-preview"
                    data-active={colorRangePreview.active}
                    viewBox="0 0 100 100"
                    preserveAspectRatio="none"
                    style={{ position: "absolute", zIndex: 2, inset: 0, width: "100%", height: "100%", overflow: "visible", pointerEvents: "none" }}
                    aria-hidden="true"
                  >
                    <line
                      x1={colorRangePreview.start.hue * 100}
                      y1={(1 - colorRangePreview.start.saturation) * 100}
                      x2={colorRangePreview.end.hue * 100}
                      y2={(1 - colorRangePreview.end.saturation) * 100}
                      stroke="white"
                      strokeWidth="1.5"
                      strokeDasharray={colorRangePreview.active ? "3 2" : undefined}
                      vectorEffect="non-scaling-stroke"
                    />
                    <circle cx={colorRangePreview.start.hue * 100} cy={(1 - colorRangePreview.start.saturation) * 100} r="2.5" fill="white" vectorEffect="non-scaling-stroke" />
                    <circle cx={colorRangePreview.end.hue * 100} cy={(1 - colorRangePreview.end.saturation) * 100} r="2.5" fill="none" stroke="white" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
                  </svg>
                )}
                <i
                  style={{
                    left: `${hue * 100}%`,
                    top: `${(1 - saturation) * 100}%`,
                  }}
                />
              </div>
              <div className="brightness-control"><span>Brightness</span><Button aria-label="Decrease brightness" onClick={() => { const value = Math.max(0, brightness - .05); setBrightness(value); void applyColors(server.selectedFixtures.map(() => ({ hue, saturation, brightness: value }))); }}>−</Button><b>{Math.round(brightness * 100)}%</b><Button aria-label="Increase brightness" onClick={() => { const value = Math.min(1, brightness + .05); setBrightness(value); void applyColors(server.selectedFixtures.map(() => ({ hue, saturation, brightness: value }))); }}>+</Button></div>
              <strong style={{ color: swatch }}>{swatch}</strong>
            </div>
          )}
          {(family === "Beam" || family === "Shapers") && (
            <div className="beam-pages">
              <header>
                <b>
                  {family} page {beamPage + 1}
                </b>
                <span className="spacer" />
                <Button
                  disabled={beamPage === 0}
                  onClick={() => setBeamPage(beamPage - 1)}
                >
                  ←
                </Button>
                <Button
                  disabled={(beamPage + 1) * 4 >= beamAttributes.length}
                  onClick={() => setBeamPage(beamPage + 1)}
                >
                  →
                </Button>
              </header>
              <div>
                {pageAttributes.length ? (
                  pageAttributes.map((attribute) => (
                    <VerticalTouchFader
                      key={attribute}
                      label={attribute.replaceAll(".", " ")}
                      value={0}
                      onChange={(value) => void apply(attribute, value / 100)}
                    />
                  ))
                ) : (
                  <p>
                    No {family.toLowerCase()} attributes exist on the selected
                    fixtures.
                  </p>
                )}
              </div>
            </div>
          )}
          {family === "Control" && (
            <div className="special-action-grid">
              <Button {...controlButtonProps("lamp_on", true)}>
                Lamps On
              </Button>
              <Button {...controlButtonProps("lamp_off")}>
                Lamp Off
              </Button>
              <Button
                className="danger"
                {...controlButtonProps("reset")}
              >
                Reset
              </Button>
              <Button {...controlButtonProps("fan_auto")}>
                Fan Auto
              </Button>
              <Button {...controlButtonProps("fan_low")}>Fan Low</Button>
              <Button {...controlButtonProps("fan_max")}>Fan Max</Button>
            </div>
          )}
          {family === "Dynamics" && (
            <VerticalTouchFader
              label="Dynamic speed"
              value={dynamicSpeed}
              maximum={240}
              display={`${dynamicSpeed} BPM`}
              onChange={(value) => {
                setDynamicSpeed(value);
                void apply("dynamic.speed", value / 240);
              }}
            />
          )}
        </div>
        {server.error && <p className="modal-error" role="alert">{server.error}</p>}
      </section>
    </div>
  );
}
