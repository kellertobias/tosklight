import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

type LogEntry = { revision: number; kind: string; payload: unknown };

export function DebugModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  useEffect(() => {
    if (!state.debugOpen) return;
    const refresh = () => void server.readServerLogs().then((entries) => setLogs(entries.slice(-200))).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 1_000);
    return () => window.clearInterval(timer);
  }, [state.debugOpen, server.readServerLogs]);
  if (!state.debugOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "debugOpen", value: false });
  return <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="nested-modal debug-modal" role="dialog" aria-modal="true" aria-label="Debug"><button className="modal-close" onClick={close}>×</button><h3>Debug</h3><div className="debug-simulators"><button className={state.midiProfile ? "active" : ""} onClick={() => dispatch({ type: "TOGGLE_MIDI_PROFILE" })}>{state.midiProfile ? "Hardware connected" : "Simulate hardware"}</button><button className={state.touchScrollbars ? "active" : ""} onClick={() => dispatch({ type: "TOGGLE_TOUCH_SCROLLBARS" })}>{state.touchScrollbars ? "Touch scrollbars forced" : "Simulate touch scrollbars"}</button><button onClick={() => server.simulateError("Simulated DMX output failure")}>Simulate DMX error</button><button onClick={() => server.simulateError(null)}>Clear simulated error</button></div><h4>Server event log</h4><pre className="server-log">{logs.length ? logs.map((entry) => `${entry.revision.toString().padStart(6, "0")}  ${entry.kind}  ${JSON.stringify(entry.payload)}`).join("\n") : "No server events logged."}</pre></section></div>;
}
