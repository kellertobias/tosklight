import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from "react";
import { GroupStrip } from "../components/shared/GroupStrip";
import { fixtures as visualFixtures } from "../data/mockData";
import { useServer } from "../api/ServerContext";
import type { WindowProps } from "./windowTypes";
import { applyMarqueeSelection, applyStageSelection } from "./stageSelection";

const symbols = ["◉", "◈", "◎", "◐", "◇", "◍"];
type StageMode = "select" | "setup" | "navigate";
type Point = { x: number; y: number };

export function StageWindow({ compact }: WindowProps) {
  const server = useServer();
  const [mode, setMode] = useState<StageMode>("select");
  const [zoom, setZoom] = useState(1);
  const [positions, setPositions] = useState<Record<string, { x: number; y: number; rotation: number }>>({});
  const positionsRef = useRef(positions);
  const [draggingFixture, setDraggingFixture] = useState<string | null>(null);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const navigationStart = useRef<(Point & { panX: number; panY: number }) | null>(null);
  const selectionAnchor = useRef<string | null>(null);
  const marqueeStart = useRef<(Point & { additive: boolean }) | null>(null);
  const [marquee, setMarquee] = useState<{ left: number; top: number; width: number; height: number } | null>(null);

  useEffect(() => { positionsRef.current = positions; }, [positions]);
  useEffect(() => { if (server.stageLayout) { positionsRef.current = server.stageLayout.body.positions; setPositions(server.stageLayout.body.positions); } }, [server.stageLayout]);
  const patched = server.patch?.fixtures.map((fixture, index) => ({ fixtureId: fixture.fixture_id, name: fixture.definition.name ?? fixture.definition.model, color: visualFixtures[index % visualFixtures.length].color, dimmer: visualFixtures[index % visualFixtures.length].dimmer })) ?? [];
  const fixtures = server.bootstrap ? patched : visualFixtures.map((fixture) => ({ fixtureId: "", name: fixture.name, color: fixture.color, dimmer: fixture.dimmer }));
  const orderedFixtureIds = fixtures.map((fixture) => fixture.fixtureId).filter(Boolean);
  const columns = compact ? 6 : 8;

  const selectFixture = (fixtureId: string, event: ReactMouseEvent<HTMLButtonElement>) => {
    if (!fixtureId || mode !== "select") return;
    const next = applyStageSelection({ fixtureId, orderedFixtureIds, selectedFixtureIds: server.selectedFixtures, anchorFixtureId: selectionAnchor.current, additive: event.ctrlKey || event.metaKey, range: event.shiftKey });
    selectionAnchor.current = fixtureId;
    void server.setSelection(next);
  };
  const moveFixture = (fixtureId: string, event: ReactPointerEvent<HTMLButtonElement>) => {
    if (mode !== "setup" || draggingFixture !== fixtureId) return;
    const bounds = event.currentTarget.parentElement?.getBoundingClientRect();
    if (!bounds) return;
    setPositions((current) => { const next = { ...current, [fixtureId]: { x: Math.max(2, Math.min(94, ((event.clientX - bounds.left) / bounds.width) * 100)), y: Math.max(3, Math.min(90, ((event.clientY - bounds.top) / bounds.height) * 100)), rotation: current[fixtureId]?.rotation ?? 0 } }; positionsRef.current = next; return next; });
  };
  const beginCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest(".stage-fixture")) return;
    if (mode === "navigate") {
      event.currentTarget.setPointerCapture(event.pointerId);
      navigationStart.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y };
    } else if (mode === "select" && event.button === 0) {
      const bounds = event.currentTarget.getBoundingClientRect();
      event.currentTarget.setPointerCapture(event.pointerId);
      marqueeStart.current = { x: event.clientX, y: event.clientY, additive: event.ctrlKey || event.metaKey };
      setMarquee({ left: event.clientX - bounds.left, top: event.clientY - bounds.top, width: 0, height: 0 });
    }
  };
  const updateCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (navigationStart.current && mode === "navigate") setPan({ x: navigationStart.current.panX + event.clientX - navigationStart.current.x, y: navigationStart.current.panY + event.clientY - navigationStart.current.y });
    const start = marqueeStart.current;
    if (!start || mode !== "select") return;
    const bounds = event.currentTarget.getBoundingClientRect();
    setMarquee({ left: Math.min(start.x, event.clientX) - bounds.left, top: Math.min(start.y, event.clientY) - bounds.top, width: Math.abs(event.clientX - start.x), height: Math.abs(event.clientY - start.y) });
  };
  const finishCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
    navigationStart.current = null;
    const start = marqueeStart.current;
    marqueeStart.current = null;
    if (!start) return setMarquee(null);
    const left = Math.min(start.x, event.clientX), right = Math.max(start.x, event.clientX), top = Math.min(start.y, event.clientY), bottom = Math.max(start.y, event.clientY);
    const moved = right - left >= 4 || bottom - top >= 4;
    if (moved) {
      const hits = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(".stage-fixture[data-fixture-id]"))
        .filter((node) => { const box = node.getBoundingClientRect(); return box.right >= left && box.left <= right && box.bottom >= top && box.top <= bottom; })
        .map((node) => node.dataset.fixtureId!).filter(Boolean);
      void server.setSelection(applyMarqueeSelection(server.selectedFixtures, hits, start.additive));
    } else if (!start.additive) void server.setSelection([]);
    setMarquee(null);
  };

  return <div className={`stage-window ${compact ? "compact" : ""}`}>
    {!compact && <header className="window-toolbar"><h1>Stage <small>{server.selectedFixtures.length} selected</small></h1><span className="spacer"/><div className="button-group"><button className={mode === "select" ? "active" : ""} onClick={() => setMode("select")}>Select fixtures</button><button className={mode === "setup" ? "active" : ""} onClick={() => setMode("setup")}>Setup positions</button><button className={mode === "navigate" ? "active" : ""} onClick={() => setMode("navigate")}>Navigate</button></div><button aria-label="Zoom out" onClick={() => setZoom(Math.max(.7, zoom - .1))}>Zoom −</button><button aria-label="Zoom in" onClick={() => setZoom(Math.min(1.6, zoom + .1))}>Zoom +</button></header>}
    <div className={`stage-canvas stage-mode-${mode}`} onPointerDown={beginCanvasGesture} onPointerMove={updateCanvasGesture} onPointerUp={finishCanvasGesture} onPointerCancel={() => { navigationStart.current = null; marqueeStart.current = null; setMarquee(null); }}>
      <div className="selection-summary"><b>{server.selectedFixtures.length} selected</b><br/><small>{mode === "select" ? "Tap to select · Shift range · Ctrl/Command add · drag marquee" : mode === "setup" ? "Drag fixtures to edit their stage positions" : "Drag the canvas to navigate"}</small></div>
      {fixtures.length === 0 && <div className="empty-window-message">No fixtures are patched in the active show.</div>}
      <div className="stage-fixture-layer" style={{ transform: `translate(${pan.x}px,${pan.y}px) scale(${zoom})` }}>{fixtures.slice(0, compact ? 18 : 24).map((fixture, index) => { const position = positions[fixture.fixtureId] ?? { x: 8 + (index % columns) * (compact ? 15 : 11.5), y: 12 + Math.floor(index / columns) * 31, rotation: index * 23 - 70 }; return <button data-fixture-id={fixture.fixtureId || undefined} onClick={(event) => selectFixture(fixture.fixtureId, event)} onPointerDown={(event) => { if (mode === "setup" && fixture.fixtureId) { event.currentTarget.setPointerCapture(event.pointerId); setDraggingFixture(fixture.fixtureId); } }} onPointerMove={(event) => moveFixture(fixture.fixtureId, event)} onPointerUp={() => { if (draggingFixture) void server.saveStageLayout({ positions: positionsRef.current }); setDraggingFixture(null); }} key={fixture.fixtureId || index} className={`stage-fixture ${server.selectedFixtures.includes(fixture.fixtureId) ? "selected" : ""}`} style={{ left: `${position.x}%`, top: `${position.y}%`, color: fixture.color, opacity: .35 + fixture.dimmer / 150 }} aria-label={`${fixture.name}, ${fixture.dimmer}%`}><span>{symbols[index % symbols.length]}</span><i style={{ transform: `rotate(${position.rotation}deg)` }}/><small>{index + 1}</small></button>; })}</div>
      {marquee && <div className="selection-marquee" style={marquee} aria-hidden="true"/>}
    </div>
    {!compact && <GroupStrip />}
  </div>;
}
