import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { Button } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";

type LogEntry = { revision: number; kind: string; payload: unknown };

export function DebugModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [debugMenuOpen, setDebugMenuOpen] = useState(false);
  useEffect(() => {
    if (!state.debugOpen) return;
    const refresh = () => void server.readServerLogs().then((entries) => setLogs(entries.slice(-200))).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 1_000);
    return () => window.clearInterval(timer);
  }, [state.debugOpen, server.readServerLogs]);
  useEffect(() => {
    if (!debugMenuOpen) return;
    const closeMenu = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setDebugMenuOpen(false);
    };
    window.addEventListener("keydown", closeMenu, true);
    return () => window.removeEventListener("keydown", closeMenu, true);
  }, [debugMenuOpen]);
  if (!state.debugOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "debugOpen", value: false });
  const closeDebugMenu = () => setDebugMenuOpen(false);
  const debugMenu = <div className="debug-title-menu">
    <Button className="debug-title-menu-trigger" aria-haspopup="menu" aria-expanded={debugMenuOpen} onClick={() => setDebugMenuOpen((open) => !open)}>Debug <span aria-hidden="true">▾</span></Button>
    {debugMenuOpen && <div className="debug-title-menu-panel" role="menu" aria-label="Debug">
      <Button role="menuitem" className={state.midiProfile ? "active" : ""} onClick={() => { dispatch({ type: "TOGGLE_MIDI_PROFILE" }); closeDebugMenu(); }}><span aria-hidden="true">{state.midiProfile ? "✓" : ""}</span>Simulate Hardware</Button>
      <Button role="menuitem" className={state.touchScrollbars ? "active" : ""} onClick={() => { dispatch({ type: "TOGGLE_TOUCH_SCROLLBARS" }); closeDebugMenu(); }}><span aria-hidden="true">{state.touchScrollbars ? "✓" : ""}</span>Simulate Touch Scroll Bars</Button>
      <Button role="menuitem" onClick={() => { server.simulateError("Simulated DMX output failure"); closeDebugMenu(); }}>Simulate DMX Error</Button>
      <Button role="menuitem" onClick={() => { server.simulateError(null); closeDebugMenu(); }}>Clear Simulated Errors</Button>
    </div>}
  </div>;
  return <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="nested-modal debug-modal" role="dialog" aria-modal="true" aria-label="Desk Status"><ModalTitleBar title="Desk Status" actions={debugMenu} closeLabel="Close Desk Status" onClose={close}/><div className="debug-diagnostics"><section><b>{server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"} Hz</b><small>Current frame rate</small></section><section><b>{server.bootstrap?.output_health.deadline_misses ?? 0}</b><small>Scheduler deadline misses</small></section><section><b>{server.bootstrap?.output_health.send_errors ?? 0}</b><small>Network output errors</small></section></div><h4>Server event log</h4><pre className="server-log">{logs.length ? logs.map((entry) => `${entry.revision.toString().padStart(6, "0")}  ${entry.kind}  ${JSON.stringify(entry.payload)}`).join("\n") : "No server events logged."}</pre></section></div>;
}
