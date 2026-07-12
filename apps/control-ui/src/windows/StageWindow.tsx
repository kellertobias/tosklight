import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type CSSProperties,
} from "react";
import { GroupStrip } from "../components/shared/GroupStrip";
import { fixtures as visualFixtures } from "../data/mockData";
import { useServer } from "../api/ServerContext";
import type { WindowProps } from "./windowTypes";
import { applyMarqueeSelection, applyStageSelection } from "./stageSelection";
import { Stage3dCanvas } from "./Stage3dCanvas";
import { fixtureValue } from "./fixtureVisualization";
import { migrateStagePosition } from "./stage3dScene";
import type { StageAsset, StagePosition3d } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { importStageAssets } from "./stageAssetImport";

const symbols = ["◉", "◈", "◎", "◐", "◇", "◍"];
type StageMode = "select" | "setup" | "navigate";
type Point = { x: number; y: number };

export function StageWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const [mode, setMode] = useState<StageMode>("select");
  const [view, setView] = useState<"2d" | "3d">("2d");
  const [followPreload, setFollowPreload] = useState(false);
  const tauri =
    typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const [zoom, setZoom] = useState(1);
  const [positions, setPositions] = useState<
    Record<string, { x: number; y: number; rotation: number }>
  >({});
  const [positions3d, setPositions3d] = useState<
    Record<string, StagePosition3d>
  >({});
  const [assets, setAssets] = useState<StageAsset[]>([]);
  const assetsRef = useRef(assets);
  const assetInput = useRef<HTMLInputElement>(null);
  const positions3dRef = useRef(positions3d);
  const [visualization, setVisualization] =
    useState<VisualizationSnapshot | null>(null);
  const positionsRef = useRef(positions);
  const [draggingFixture, setDraggingFixture] = useState<string | null>(null);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const navigationStart = useRef<
    (Point & { panX: number; panY: number }) | null
  >(null);
  const selectionAnchor = useRef<string | null>(null);
  const marqueeStart = useRef<(Point & { additive: boolean }) | null>(null);
  const [marquee, setMarquee] = useState<{
    left: number;
    top: number;
    width: number;
    height: number;
  } | null>(null);

  useEffect(() => {
    positionsRef.current = positions;
  }, [positions]);
  useEffect(() => {
    positions3dRef.current = positions3d;
  }, [positions3d]);
  useEffect(() => {
    assetsRef.current = assets;
  }, [assets]);
  useEffect(() => {
    if (server.stageLayout) {
      positionsRef.current = server.stageLayout.body.positions;
      setPositions(server.stageLayout.body.positions);
      positions3dRef.current = server.stageLayout.body.positions3d ?? {};
      setPositions3d(server.stageLayout.body.positions3d ?? {});
      assetsRef.current = server.stageLayout.body.assets ?? [];
      setAssets(server.stageLayout.body.assets ?? []);
    }
  }, [server.stageLayout]);
  useEffect(() => {
    let cancelled = false;
    const refresh = () =>
      void server
        .readVisualization(followPreload)
        .then((next) => {
          if (!cancelled) setVisualization(next);
        })
        .catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 200);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [followPreload, server.readVisualization]);
  const patched =
    server.patch?.fixtures.map((fixture) => {
      const intensity = fixtureValue(visualization, fixture, "intensity");
      const red = fixtureValue(visualization, fixture, "color.red", 1);
      const green = fixtureValue(visualization, fixture, "color.green", 1);
      const blue = fixtureValue(visualization, fixture, "color.blue", 1);
      const panValue = fixtureValue(visualization, fixture, "pan");
      const tiltValue = fixtureValue(visualization, fixture, "tilt");
      return ({
      fixtureId: fixture.fixture_id,
      name: fixture.definition.name ?? fixture.definition.model,
      icon: fixture.definition.icon_asset,
      color: `rgb(${Math.round(red * 255)},${Math.round(green * 255)},${Math.round(blue * 255)})`,
      dimmer: Math.round(intensity * 100),
      pan: panValue,
      tilt: tiltValue,
    });}) ?? [];
  const fixtures = server.bootstrap
    ? patched
    : visualFixtures.map((fixture) => ({
        fixtureId: "",
        name: fixture.name,
        icon: null,
        color: fixture.color,
        dimmer: fixture.dimmer,
        pan: Math.max(0, Math.min(1, fixture.pan / 360)),
        tilt: Math.max(0, Math.min(1, fixture.tilt / 180)),
      }));
  const orderedFixtureIds = fixtures
    .map((fixture) => fixture.fixtureId)
    .filter(Boolean);
  const columns = compact ? 6 : 8;
  const fixtures3d = useMemo(
    () =>
      (server.patch?.fixtures ?? []).map((fixture, index) => ({
        fixture,
        index,
        position:
          positions3d[fixture.fixture_id] ??
          migrateStagePosition(positions[fixture.fixture_id], index),
      })),
    [server.patch, positions3d, positions],
  );
  const save3d = () =>
    server.saveStageLayout({
      version: 2,
      positions: positionsRef.current,
      positions3d: positions3dRef.current,
      assets: assetsRef.current,
    });
  const importAssets = async (file: File) => {
    const imported = await importStageAssets(file);
    const next = [...assetsRef.current, ...imported];
    assetsRef.current = next;
    setAssets(next);
    await server.saveStageLayout({
      version: 2,
      positions: positionsRef.current,
      positions3d: positions3dRef.current,
      assets: next,
    });
  };

  const selectFixture = (
    fixtureId: string,
    event: ReactMouseEvent<HTMLButtonElement>,
  ) => {
    if (!fixtureId || mode !== "select") return;
    const next = applyStageSelection({
      fixtureId,
      orderedFixtureIds,
      selectedFixtureIds: server.selectedFixtures,
      anchorFixtureId: selectionAnchor.current,
      additive: event.ctrlKey || event.metaKey,
      range: event.shiftKey,
    });
    selectionAnchor.current = fixtureId;
    void server.setSelection(next);
  };
  const moveFixture = (
    fixtureId: string,
    event: ReactPointerEvent<HTMLButtonElement>,
  ) => {
    if (mode !== "setup" || draggingFixture !== fixtureId) return;
    const bounds = event.currentTarget.parentElement?.getBoundingClientRect();
    if (!bounds) return;
    setPositions((current) => {
      const next = {
        ...current,
        [fixtureId]: {
          x: Math.max(
            2,
            Math.min(94, ((event.clientX - bounds.left) / bounds.width) * 100),
          ),
          y: Math.max(
            3,
            Math.min(90, ((event.clientY - bounds.top) / bounds.height) * 100),
          ),
          rotation: current[fixtureId]?.rotation ?? 0,
        },
      };
      positionsRef.current = next;
      return next;
    });
  };
  const beginCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest(".stage-fixture")) return;
    if (mode === "navigate") {
      event.currentTarget.setPointerCapture(event.pointerId);
      navigationStart.current = {
        x: event.clientX,
        y: event.clientY,
        panX: pan.x,
        panY: pan.y,
      };
    } else if (mode === "select" && event.button === 0) {
      const bounds = event.currentTarget.getBoundingClientRect();
      event.currentTarget.setPointerCapture(event.pointerId);
      marqueeStart.current = {
        x: event.clientX,
        y: event.clientY,
        additive: event.ctrlKey || event.metaKey,
      };
      setMarquee({
        left: event.clientX - bounds.left,
        top: event.clientY - bounds.top,
        width: 0,
        height: 0,
      });
    }
  };
  const updateCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (navigationStart.current && mode === "navigate")
      setPan({
        x:
          navigationStart.current.panX +
          event.clientX -
          navigationStart.current.x,
        y:
          navigationStart.current.panY +
          event.clientY -
          navigationStart.current.y,
      });
    const start = marqueeStart.current;
    if (!start || mode !== "select") return;
    const bounds = event.currentTarget.getBoundingClientRect();
    setMarquee({
      left: Math.min(start.x, event.clientX) - bounds.left,
      top: Math.min(start.y, event.clientY) - bounds.top,
      width: Math.abs(event.clientX - start.x),
      height: Math.abs(event.clientY - start.y),
    });
  };
  const finishCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    navigationStart.current = null;
    const start = marqueeStart.current;
    marqueeStart.current = null;
    if (!start) return setMarquee(null);
    const left = Math.min(start.x, event.clientX),
      right = Math.max(start.x, event.clientX),
      top = Math.min(start.y, event.clientY),
      bottom = Math.max(start.y, event.clientY);
    const moved = right - left >= 4 || bottom - top >= 4;
    if (moved) {
      const hits = Array.from(
        event.currentTarget.querySelectorAll<HTMLElement>(
          ".stage-fixture[data-fixture-id]",
        ),
      )
        .filter((node) => {
          const box = node.getBoundingClientRect();
          return (
            box.right >= left &&
            box.left <= right &&
            box.bottom >= top &&
            box.top <= bottom
          );
        })
        .map((node) => node.dataset.fixtureId!)
        .filter(Boolean);
      void server.setSelection(
        applyMarqueeSelection(server.selectedFixtures, hits, start.additive),
      );
    } else if (!start.additive) void server.setSelection([]);
    setMarquee(null);
  };

  return (
    <div className={`stage-window ${compact ? "compact" : ""}`}>
      {compact && (
        <button
          className={`stage-follow-preload ${followPreload ? "active" : ""}`}
          onClick={() => setFollowPreload(!followPreload)}
        >
          Follow Preload
        </button>
      )}
      {!compact && (
        <header className="window-toolbar">
          <h1>
            Stage <small>{server.selectedFixtures.length} selected</small>
          </h1>
          <span className="spacer" />
          <button className={followPreload ? "active" : ""} onClick={() => setFollowPreload(!followPreload)}>Follow Preload</button>
          {tauri && (
            <div className="button-group">
              <button
                className={view === "2d" ? "active" : ""}
                onClick={() => setView("2d")}
              >
                2D
              </button>
              <button
                className={view === "3d" ? "active" : ""}
                onClick={() => setView("3d")}
              >
                3D
              </button>
            </div>
          )}
          <div className="button-group">
            <button
              className={mode === "select" ? "active" : ""}
              onClick={() => setMode("select")}
            >
              Select fixtures
            </button>
            <button
              className={mode === "setup" ? "active" : ""}
              onClick={() => setMode("setup")}
            >
              Setup positions
            </button>
            <button
              className={mode === "navigate" ? "active" : ""}
              onClick={() => setMode("navigate")}
            >
              Navigate
            </button>
          </div>
          {view === "3d" && (
            <>
              <input
                ref={assetInput}
                hidden
                type="file"
                accept=".glb,.stl,.3mf,.mvr,.gdtf"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) void importAssets(file);
                  event.currentTarget.value = "";
                }}
              />
              <button onClick={() => assetInput.current?.click()}>Import scene</button>
            </>
          )}
          {view === "2d" && (
            <>
              <button
                aria-label="Zoom out"
                onClick={() => setZoom(Math.max(0.7, zoom - 0.1))}
              >
                Zoom −
              </button>
              <button
                aria-label="Zoom in"
                onClick={() => setZoom(Math.min(1.6, zoom + 0.1))}
              >
                Zoom +
              </button>
            </>
          )}
        </header>
      )}
      {view === "3d" ? (
        <div className="stage-canvas stage-canvas-3d">
          <Stage3dCanvas
            fixtures={fixtures3d}
            assets={assets}
            visualization={visualization}
            selected={server.selectedFixtures}
            setup={mode === "setup"}
            onSelect={(fixtureId, additive) =>
              void server.setSelection(
                additive
                  ? applyMarqueeSelection(
                      server.selectedFixtures,
                      [fixtureId],
                      true,
                    )
                  : [fixtureId],
              )
            }
            onMove={(fixtureId, position) =>
              setPositions3d((current) => {
                const next = { ...current, [fixtureId]: position };
                positions3dRef.current = next;
                return next;
              })
            }
            onMoveEnd={(fixtureId, position) => {
              const next = { ...positions3dRef.current, [fixtureId]: position };
              positions3dRef.current = next;
              void server.saveStageLayout({ version: 2, positions: positionsRef.current, positions3d: next, assets: assetsRef.current });
            }}
          />
          {mode === "setup" &&
            server.selectedFixtures[0] &&
            (() => {
              const id = server.selectedFixtures[0];
              const fixture = fixtures3d.find(
                (item) => item.fixture.fixture_id === id,
              );
              if (!fixture) return null;
              const update = (key: keyof StagePosition3d, value: number) => {
                const next = { ...fixture.position, [key]: value };
                setPositions3d((current) => {
                  const result = { ...current, [id]: next };
                  positions3dRef.current = result;
                  return result;
                });
              };
              return (
                <aside className="stage-3d-inspector">
                  <b>Fixture position</b>
                  {(
                    [
                      "x",
                      "y",
                      "z",
                      "rotationX",
                      "rotationY",
                      "rotationZ",
                    ] as const
                  ).map((key) => (
                    <label key={key}>
                      {key}
                      <input
                        type="number"
                        step={key.startsWith("rotation") ? 1 : 0.1}
                        value={fixture.position[key]}
                        onChange={(event) =>
                          update(key, Number(event.target.value))
                        }
                        onBlur={() => void save3d()}
                      />
                    </label>
                  ))}
                </aside>
              );
            })()}
        </div>
      ) : (
        <div
          className={`stage-canvas stage-mode-${mode}`}
          onPointerDown={beginCanvasGesture}
          onPointerMove={updateCanvasGesture}
          onPointerUp={finishCanvasGesture}
          onPointerCancel={() => {
            navigationStart.current = null;
            marqueeStart.current = null;
            setMarquee(null);
          }}
        >
          <div className="selection-summary">
            <b>{server.selectedFixtures.length} selected</b>
            <br />
            <small>
              {mode === "select"
                ? "Tap to select · Shift range · Ctrl/Command add · drag marquee"
                : mode === "setup"
                  ? "Drag fixtures to edit their stage positions"
                  : "Drag the canvas to navigate"}
            </small>
          </div>
          {fixtures.length === 0 && (
            <div className="empty-window-message">
              No fixtures are patched in the active show.
            </div>
          )}
          <div
            className="stage-fixture-layer"
            style={{
              transform: `translate(${pan.x}px,${pan.y}px) scale(${zoom})`,
            }}
          >
            {fixtures.slice(0, compact ? 18 : 24).map((fixture, index) => {
              const position = positions[fixture.fixtureId] ?? {
                x: 8 + (index % columns) * (compact ? 15 : 11.5),
                y: 12 + Math.floor(index / columns) * 31,
                rotation: index * 23 - 70,
              };
              return (
                <button
                  data-fixture-id={fixture.fixtureId || undefined}
                  onClick={(event) => selectFixture(fixture.fixtureId, event)}
                  onPointerDown={(event) => {
                    if (mode === "setup" && fixture.fixtureId) {
                      event.currentTarget.setPointerCapture(event.pointerId);
                      setDraggingFixture(fixture.fixtureId);
                    }
                  }}
                  onPointerMove={(event) =>
                    moveFixture(fixture.fixtureId, event)
                  }
                  onPointerUp={() => {
                    if (draggingFixture)
                      void server.saveStageLayout({
                        version: 2,
                        positions: positionsRef.current,
                        positions3d: positions3dRef.current,
                        assets: assetsRef.current,
                      });
                    setDraggingFixture(null);
                  }}
                  key={fixture.fixtureId || index}
                  className={`stage-fixture ${server.selectedFixtures.includes(fixture.fixtureId) ? "selected" : ""}`}
                  style={{
                    left: `${position.x}%`,
                    top: `${position.y}%`,
                    color: fixture.color,
                    "--lamp-fill": `${12 + fixture.dimmer * .36}%`,
                    "--lamp-ring": `${20 + fixture.dimmer * .65}%`,
                  } as CSSProperties}
                  aria-label={`${fixture.name}, ${fixture.dimmer}%`}
                >
                  <span>{fixture.icon ? <img src={fixture.icon} alt="" /> : symbols[index % symbols.length]}<i className="lamp-color-dot" style={{ background: fixture.color }} /></span>
                  <i className="lamp-position-line" style={{ transform: `rotate(${fixture.pan * 360 - 180}deg)` }}><i style={{ left: `${fixture.tilt * 100}%` }} /></i>
                  <small>{index + 1}</small>
                </button>
              );
            })}
          </div>
          {marquee && (
            <div
              className="selection-marquee"
              style={marquee}
              aria-hidden="true"
            />
          )}
        </div>
      )}
      {(!compact || showGroupShortcuts) && <GroupStrip />}
    </div>
  );
}
