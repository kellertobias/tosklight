import { useRef, type CSSProperties } from "react";
import type { PaneModel } from "../../types";
import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";

export function Pane({
  pane,
  maximized,
  editing,
}: {
  pane: PaneModel;
  maximized: boolean;
  editing: boolean;
}) {
  const { dispatch } = useApp();
  const drag = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const resize = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const Window = windowRegistry[pane.kind];
  const style = {
    gridColumn: `${pane.x} / span ${pane.width}`,
    gridRow: `${pane.y} / span ${pane.height}`,
  } as CSSProperties;
  return (
    <article
      className={`desk-pane ${maximized ? "maximized" : ""} ${editing ? "editing" : ""}`}
      style={style}
    >
    <header className="pane-drag-handle" onPointerDown={(event) => { if ((event.target as HTMLElement).closest("button")) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; drag.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { const active = drag.current; if (!active || active.pointerId !== event.pointerId) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; const x = Math.max(1, Math.min(24 - pane.width + 1, Math.floor((event.clientX - active.left) / grid.width * 24) + 1)); const y = Math.max(1, Math.min(18 - pane.height + 1, Math.floor((event.clientY - active.top) / grid.height * 18) + 1)); if (x !== pane.x || y !== pane.y) dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { x, y } }); }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
      <b>{pane.title}</b>
      {pane.kind === "fixtures" && <span className="source-legend"><i className="source-programmer">● Programmer</i><i className="source-playback">● Playback</i><i className="source-default">● Default</i></span>}
      <span className="spacer" />
        <button
          aria-label={`Maximize ${pane.title}`}
          onClick={() => dispatch({ type: "TOGGLE_MAXIMIZE", id: pane.id })}
        >
          ↗
        </button>
        <button
          aria-label={`Pane settings for ${pane.title}`}
          onClick={() => dispatch({ type: "SET_PANE_SETTINGS", id: pane.id })}
        >
          ⚙
        </button>
      </header>
      <div className="pane-content">
        <Window compact showGroupShortcuts={Boolean(pane.showGroupShortcuts)} />
      </div>
      {!maximized && <div className="pane-resize-handle" aria-label={`Resize ${pane.title}`} onPointerDown={(event) => { event.stopPropagation(); const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; resize.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { const active = resize.current; if (!active || active.pointerId !== event.pointerId) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; const right = Math.max(pane.x, Math.min(24, Math.ceil((event.clientX - active.left) / grid.width * 24))); const bottom = Math.max(pane.y, Math.min(18, Math.ceil((event.clientY - active.top) / grid.height * 18))); dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width: right - pane.x + 1, height: bottom - pane.y + 1 } }); }} onPointerUp={() => { resize.current = null; }} onPointerCancel={() => { resize.current = null; }} />}
    </article>
  );
}
