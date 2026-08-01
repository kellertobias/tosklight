import { useEffect, useRef, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useOutputHealth } from "../../features/deskSnapshot/DeskSnapshotState";
import { useShellStatusActions } from "../../features/shellStatus/ShellStatusActionsProvider";
import { Button, ModalPortal } from "@tosklight/ui";
import { ModalTitleBar } from "@tosklight/ui";

type LogEntry = { revision: number; kind: string; payload: unknown };

const MAJOR_DESK_EVENT_KINDS = new Set([
  "session_started",
  "session_disconnected",
  "hardware_connection_changed",
  "media_server_offline",
  "preload_persistence_failed",
]);

export function isMajorDeskEvent(entry: LogEntry) {
  const kind = entry.kind.toLowerCase();
  if (MAJOR_DESK_EVENT_KINDS.has(kind)) return true;
  return /(?:^|_)(?:error|failed|failure|rejected|disconnected|offline)(?:_|$)/.test(kind);
}

export function DebugModal() {
  const { state, dispatch } = useApp();
  const shellStatus = useShellStatusActions();
  const outputHealth = useOutputHealth();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const lastRevision = useRef(0);
  const reading = useRef(false);
  const [debugMenuOpen, setDebugMenuOpen] = useState(false);
  useEffect(() => {
    if (!state.debugOpen || !shellStatus) return;
    let cancelled = false;
    lastRevision.current = 0;
    setLogs([]);
    const refresh = async () => {
      if (reading.current) return;
      reading.current = true;
      try {
        const entries = await shellStatus.readServerLogs(lastRevision.current);
        if (cancelled) return;
        if (entries.length)
          lastRevision.current = Math.max(
            lastRevision.current,
            ...entries.map((entry) => entry.revision),
          );
        const major = entries.filter(isMajorDeskEvent);
        if (major.length)
          setLogs((current) => {
            const byRevision = new Map(
              [...current, ...major].map((entry) => [entry.revision, entry]),
            );
            return [...byRevision.values()]
              .sort((left, right) => left.revision - right.revision)
              .slice(-50);
          });
      } catch {
        // Desk Status remains useful even if one diagnostic refresh fails.
      } finally {
        reading.current = false;
      }
    };
    refresh();
    const timer = window.setInterval(() => void refresh(), 5_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [state.debugOpen, shellStatus]);
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
      <Button role="menuitem" className={state.showSectionNames ? "active" : ""} aria-pressed={state.showSectionNames} onClick={() => {
        dispatch({ type: "TOGGLE_SECTION_NAMES" });
        dispatch({ type: "SET_MODAL", modal: "debugOpen", value: false });
        dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
        closeDebugMenu();
      }}><span aria-hidden="true">{state.showSectionNames ? "✓" : ""}</span>Show section names</Button>
      <Button role="menuitem" className={state.midiProfile ? "active" : ""} onClick={() => { dispatch({ type: "TOGGLE_MIDI_PROFILE" }); closeDebugMenu(); }}><span aria-hidden="true">{state.midiProfile ? "✓" : ""}</span>Simulate Hardware</Button>
      <Button role="menuitem" className={state.touchScrollbars ? "active" : ""} onClick={() => { dispatch({ type: "TOGGLE_TOUCH_SCROLLBARS" }); closeDebugMenu(); }}><span aria-hidden="true">{state.touchScrollbars ? "✓" : ""}</span>Simulate Touch Scroll Bars</Button>
      <Button role="menuitem" onClick={() => { shellStatus?.simulateError("Simulated DMX output failure"); closeDebugMenu(); }}>Simulate DMX Error</Button>
      <Button role="menuitem" onClick={() => { shellStatus?.simulateError(null); closeDebugMenu(); }}>Clear Simulated Errors</Button>
    </div>}
  </div>;
  return <ModalPortal onClose={close}><div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="nested-modal debug-modal" role="dialog" aria-modal="true" aria-label="Desk Status"><ModalTitleBar title="Desk Status" actions={debugMenu} closeLabel="Close Desk Status" onClose={close}/><div className="debug-diagnostics"><section><b>{outputHealth?.frame_hz.toFixed(1) ?? "—"} Hz</b><small>Current frame rate</small></section><section><b>{outputHealth?.deadline_misses ?? 0}</b><small>Scheduler deadline misses</small></section><section><b>{outputHealth?.send_errors ?? 0}</b><small>Network output errors</small></section></div><h4>Major desk events</h4><pre className="server-log">{logs.length ? logs.map((entry) => `${entry.revision.toString().padStart(6, "0")}  ${entry.kind}  ${JSON.stringify(entry.payload)}`).join("\n") : "No major desk events logged."}</pre></section></div></ModalPortal>;
}
