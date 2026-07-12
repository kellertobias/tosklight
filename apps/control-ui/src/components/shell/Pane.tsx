import type { CSSProperties } from "react";
import type { PaneModel } from "../../types";
import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";

export function Pane({ pane, maximized, editing }: { pane: PaneModel; maximized: boolean; editing: boolean }) {
  const { dispatch } = useApp();
  const Window = windowRegistry[pane.kind];
  const style = { gridColumn: `${pane.x} / span ${pane.width}`, gridRow: `${pane.y} / span ${pane.height}` } as CSSProperties;
  return <article className={`desk-pane ${maximized ? "maximized" : ""} ${editing ? "editing" : ""}`} style={style}>
    <header><b>{pane.title}</b><span className="spacer"/><button aria-label={`Maximize ${pane.title}`} onClick={() => dispatch({ type: "TOGGLE_MAXIMIZE", id: pane.id })}>↗</button><button aria-label={`Pane settings for ${pane.title}`} onClick={() => dispatch({ type: "SET_PANE_SETTINGS", id: pane.id })}>⚙</button></header>
    <div className="pane-content"><Window compact /></div>
  </article>;
}
