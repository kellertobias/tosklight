import { useRef, useState, type CSSProperties } from "react";
import type { PaneModel } from "../../types";
import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";
import { useServer } from "../../api/ServerContext";
import { Button } from "../common";
import { WindowHeader } from "../window-kit";
import { SourceLegend } from "../shared/SourceLegend";
import { PaneChromeProvider } from "./PaneChromeContext";

export function Pane({
  pane,
  maximized,
  editing,
}: {
  pane: PaneModel;
  maximized: boolean;
  editing: boolean;
}) {
  const { state, dispatch } = useApp();
  const server = useServer();
  const drag = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const resize = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const lastFollowToggle = useRef(0);
  const [chromeInfo, setChromeInfo] = useState<HTMLSpanElement | null>(null);
  const [chromeToolbar, setChromeToolbar] = useState<HTMLSpanElement | null>(null);
  const Window = windowRegistry[pane.kind];
  const style = {
    gridColumn: `${pane.x} / span ${pane.width}`,
    gridRow: `${pane.y} / span ${pane.height}`,
  } as CSSProperties;
  const stageActions = pane.kind === "stage" ? [
    ...(state.stageMode === "setup" ? [] : [[{ id: "follow", label: "Follow Preload", active: Boolean(pane.followPreload), onClick: () => { const now = performance.now(); if (now - lastFollowToggle.current < 400) return; lastFollowToggle.current = now; dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: !pane.followPreload }); } }]]),
    [{ id: "groups", label: "Groups", onClick: () => dispatch({ type: "OPEN_GROUPS_FROM_STAGE", origin: "desk" }) }],
  ] : [];
  return (
    <article
      className={`desk-pane ${maximized ? "maximized" : ""} ${editing ? "editing" : ""}`}
      style={style}
    >
    <WindowHeader title={pane.kind === "file_manager" ? "File Manager" : pane.title} info={pane.kind === "file_manager" ? { primary: "Browse and manage files", secondary: <span className="pane-chrome-info-target" ref={setChromeInfo} /> } : pane.kind === "text_editor" ? { primary: <span className="pane-chrome-info-target" ref={setChromeInfo} /> } : pane.kind === "stage" ? { primary: `${server.selectedFixtures.length} selected`, secondary: "Tap to select · Shift for range" } : pane.kind === "fixtures" ? { primary: `${server.selectedFixtures.length} selected`, secondary: <SourceLegend /> } : undefined} toolbar={pane.kind === "file_manager" || pane.kind === "text_editor" ? <span className="pane-chrome-toolbar-target" ref={setChromeToolbar} /> : undefined} actions={stageActions} settings={pane.kind !== "file_manager"} onSettings={() => dispatch({ type: "SET_PANE_SETTINGS", id: pane.id })} dragHandleProps={{ className: "pane-drag-handle", onPointerDown: (event) => { if ((event.target as HTMLElement).closest("button")) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; drag.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }, onPointerMove: (event) => { const active = drag.current; if (!active || active.pointerId !== event.pointerId) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; const x = Math.max(1, Math.min(24 - pane.width + 1, Math.floor((event.clientX - active.left) / grid.width * 24) + 1)); const y = Math.max(1, Math.min(18 - pane.height + 1, Math.floor((event.clientY - active.top) / grid.height * 18) + 1)); if (x !== pane.x || y !== pane.y) dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { x, y } }); }, onPointerUp: () => { drag.current = null; }, onPointerCancel: () => { drag.current = null; } }} />
      <div className="pane-content">
        <PaneChromeProvider value={{ info: chromeInfo, toolbar: chromeToolbar }}>
          <Window compact paneId={pane.id} showGroupShortcuts={Boolean(pane.showGroupShortcuts)} stageView={pane.stageView ?? state.stageView} followPreload={Boolean(pane.followPreload)} showBeamGuides={pane.showBeamGuides ?? true} presetFamily={pane.presetFamily ?? state.presetFamily} presetPoolColors={pane.presetPoolColors ?? true} developmentView={pane.developmentView ?? "forms"} />
        </PaneChromeProvider>
      </div>
      {!maximized && <div className="pane-resize-handle" aria-label={`Resize ${pane.title}`} onPointerDown={(event) => { event.stopPropagation(); const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; resize.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { const active = resize.current; if (!active || active.pointerId !== event.pointerId) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; const right = Math.max(pane.x, Math.min(24, Math.ceil((event.clientX - active.left) / grid.width * 24))); const bottom = Math.max(pane.y, Math.min(18, Math.ceil((event.clientY - active.top) / grid.height * 18))); dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width: right - pane.x + 1, height: bottom - pane.y + 1 } }); }} onPointerUp={() => { resize.current = null; }} onPointerCancel={() => { resize.current = null; }} />}
    </article>
  );
}
