import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function SystemControlsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [master, setMaster] = useState(100);
  const [blackout, setBlackout] = useState(false);
  if (!state.systemControlsOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: false });
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><section className="modal-card system-controls-card">
    <button className="modal-close" onClick={close}>×</button><h2>Output & Programmers</h2>
    <section className="master-controls"><label>Grand master <strong>{master}%</strong><input aria-label="Grand master" type="range" min="0" max="100" value={master} onChange={(event) => { const value = Number(event.target.value); setMaster(value); void server.setMaster(value / 100, undefined); }}/></label><button className={blackout ? "danger active" : "danger"} onClick={() => { const next = !blackout; setBlackout(next); void server.setMaster(undefined, next); }}>{blackout ? "RELEASE BLACKOUT" : "BLACKOUT"}</button></section>
    <h3>Active programmers</h3><div className="programmer-list">{server.bootstrap?.active_programmers.map((programmer) => <article key={programmer.session_id}><span><b>{programmer.session_id === server.session?.session_id ? `${server.session.user.name} · This session` : `User ${programmer.user_id.slice(0, 8)}`}</b><small>{programmer.selected.length} fixtures · {programmer.values.length} values · {programmer.connected ? "Connected" : "Disconnected"}</small></span><button className="danger" onClick={() => void server.clearProgrammer(programmer.session_id)}>Clear</button></article>)}{!server.bootstrap?.active_programmers.length && <p className="empty-window-message">No active programmers.</p>}</div>
  </section></div>;
}
