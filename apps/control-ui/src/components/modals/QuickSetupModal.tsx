import { useRef, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

export function QuickSetupModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [showName, setShowName] = useState("");
  const [transition, setTransition] = useState<"safe_blackout" | "hold_current" | "timed_fade">("safe_blackout");
  const [overwrite, setOverwrite] = useState(false);
  const [confirmShutdown, setConfirmShutdown] = useState(false);
  const upload = useRef<HTMLInputElement>(null);
  if (!state.setupOpen) return null;

  const close = () => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.currentTarget === event.target) close(); }}><section className="modal-card show-modal">
    <button className="modal-close" onClick={close}>×</button>
    <h2>Show & Desk Setup</h2>
    <div className="show-details"><b>{server.bootstrap?.active_show?.name ?? "No active show"}</b><span>Server <strong>{server.status}</strong></span><span>Show revision <strong>{server.bootstrap?.active_show?.revision ?? "—"}</strong></span><span>Operator <strong>{server.session?.user.name ?? "—"}</strong></span></div>
    <div className="show-create-row">
      <input aria-label="Show name" value={showName} onChange={(event) => setShowName(event.target.value)} placeholder="New show name"/>
      <button disabled={!showName.trim()} onClick={() => { void server.createShow(showName.trim()); setShowName(""); }}>New Show</button>
      <button disabled={!showName.trim() || !server.bootstrap?.active_show} onClick={() => { void server.saveShowAs(showName.trim()); setShowName(""); }}>Save As</button>
      <button onClick={() => upload.current?.click()}>Upload</button>
      <button className={overwrite ? "active" : ""} onClick={() => setOverwrite(!overwrite)}>{overwrite ? "Overwrite ON" : "Overwrite OFF"}</button>
      <input ref={upload} hidden type="file" accept=".show,application/x-sqlite3" onChange={(event) => { const file = event.target.files?.[0]; if (file) void server.uploadShow(file, overwrite); event.target.value = ""; }}/>
    </div>
    <div className="show-transition"><label>Activation<select value={transition} onChange={(event) => setTransition(event.target.value as typeof transition)}><option value="safe_blackout">Safe blackout</option><option value="hold_current">Hold current</option><option value="timed_fade">Timed fade</option></select></label><button onClick={() => void server.rollbackShow()}>Rollback previous show</button></div>
    <div className="modal-actions"><button disabled={!server.bootstrap?.active_show} onClick={() => void server.saveDeskLayout({ desks: state.desks, activeDeskId: state.activeDeskId })}>Quick Save Desk</button><button disabled={!server.bootstrap?.active_show} onClick={server.exportPaperwork}>Export Paperwork</button></div>
    <div className="show-library">{server.shows.map((show) => <article className={server.bootstrap?.active_show?.id === show.id ? "active" : ""} key={show.id}><span><b>{show.name}</b><small>Revision {show.revision} · {new Date(show.updated_at).toLocaleString()}</small></span><button disabled={server.bootstrap?.active_show?.id === show.id} onClick={() => void server.openShow(show.id, transition)}>Open</button><button onClick={() => void server.downloadShow(show)}>Download</button></article>)}</div>
    <div className="modal-actions three"><button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "fixtures" }); close(); }}>Show Patch</button><button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "setup" }); close(); }}>Enter Setup</button><button className="danger" onClick={() => setConfirmShutdown(true)}>Shutdown Desk</button></div>
    {confirmShutdown && <div className="danger-confirm" role="alertdialog" aria-label="Confirm server shutdown"><span><b>Shut down the lighting server?</b><small>Hazardous fixtures will be driven to their safe values and protocol termination frames will be sent.</small></span><button onClick={() => setConfirmShutdown(false)}>Cancel</button><button className="danger" onClick={() => void server.shutdownServer()}>Shut Down Safely</button></div>}
    <button className="profile-button" onClick={() => dispatch({ type: "TOGGLE_MIDI_PROFILE" })}>{state.midiProfile ? "Use touch-only profile" : "Use MIDI controller profile"}</button>
    {server.error && <p className="modal-error">{server.error}</p>}
  </section></div>;
}
