import { useEffect, useMemo, useRef, useState, type PointerEvent, type RefObject } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "../control/VerticalTouchFader";
import { moveLampPositions, resolveLampPositions } from "./specialPosition";
import { Button } from "../common";

function hsvToRgb(h: number, s: number, v: number) {
  const i = Math.floor(h * 6),
    f = h * 6 - i,
    p = v * (1 - s),
    q = v * (1 - f * s),
    t = v * (1 - (1 - f) * s);
  return (
    [
      [v, t, p],
      [q, v, p],
      [p, v, t],
      [p, q, v],
      [t, p, v],
      [v, p, q],
    ] as number[][]
  )[i % 6];
}

export function SpecialDialogsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [pan, setPan] = useState(0.5),
    [tilt, setTilt] = useState(0.5);
  const [hue, setHue] = useState(0.52),
    [saturation, setSaturation] = useState(0.8),
    [brightness, setBrightness] = useState(0.85);
  const [beamPage, setBeamPage] = useState(0),
    [dynamicSpeed, setDynamicSpeed] = useState(30);
  const trackball = useRef<HTMLDivElement>(null),
    colorSheet = useRef<HTMLDivElement>(null);
  const joystick = useRef({ x: 0, y: 0 });
  const fixturePositions = useRef(new Map<string, { pan: number; tilt: number }>());
  const selectedFixtureKey = server.selectedFixtures.join("\u0000");
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
  const apply = async (attribute: string, value: number) =>
    Promise.all(
      server.selectedFixtures.map((fixture) =>
        server.setProgrammer(fixture, attribute, value),
      ),
    );
  const applyColor = async (h = hue, s = saturation, v = brightness) => {
    const [red, green, blue] = hsvToRgb(h, s, v);
    await Promise.all(
      server.selectedFixtures.flatMap((fixture) => [
        server.setProgrammer(fixture, "color.red", red),
        server.setProgrammer(fixture, "color.green", green),
        server.setProgrammer(fixture, "color.blue", blue),
      ]),
    );
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
  const moveColor = (event: PointerEvent<HTMLDivElement>) => {
    const next = point(event, colorSheet);
    setHue(next.x);
    setSaturation(1 - next.y);
    void applyColor(next.x, 1 - next.y, brightness);
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
  const color = hsvToRgb(hue, saturation, brightness);
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
              <span className="position-trackball-readout">Relative move<br/><b>Avg Pan {Math.round(pan * 100)}%</b><b>Avg Tilt {Math.round(tilt * 100)}%</b></span>
            </div>
          )}
          {family === "Color" && (
            <div className="graphical-color-picker">
              <div
                ref={colorSheet}
                className="color-sheet"
                style={{ backgroundColor: `hsl(${hue * 360} 100% 50%)` }}
                onPointerDown={(event) => {
                  event.currentTarget.setPointerCapture(event.pointerId);
                  moveColor(event);
                }}
                onPointerMove={(event) => {
                  if (event.currentTarget.hasPointerCapture(event.pointerId))
                    moveColor(event);
                }}
              >
                <i
                  style={{
                    left: `${hue * 100}%`,
                    top: `${(1 - saturation) * 100}%`,
                  }}
                />
              </div>
              <div className="brightness-control"><span>Brightness</span><Button aria-label="Decrease brightness" onClick={() => { const value = Math.max(0, brightness - .05); setBrightness(value); void applyColor(hue, saturation, value); }}>−</Button><b>{Math.round(brightness * 100)}%</b><Button aria-label="Increase brightness" onClick={() => { const value = Math.min(1, brightness + .05); setBrightness(value); void applyColor(hue, saturation, value); }}>+</Button></div>
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
              <Button onClick={() => void apply("control.lamp", 1)}>
                Lamp On
              </Button>
              <Button onClick={() => void apply("control.lamp", 0)}>
                Lamp Off
              </Button>
              <Button
                className="danger"
                onClick={() => void apply("control.reset", 1)}
              >
                Reset
              </Button>
              <Button onClick={() => void apply("control.fan", 0.5)}>
                Fan Auto
              </Button>
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
      </section>
    </div>
  );
}
