import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { DebugModal } from "./DebugModal";
import { ModalTextKeyboard } from "../input/ModalInputControls";

export function QuickSetupModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [showName, setShowName] = useState("");
  const [saveAsOpen, setSaveAsOpen] = useState(false);
  const [loadOpen, setLoadOpen] = useState(false);
  const [confirmShutdown, setConfirmShutdown] = useState(false);
  const [changeUserOpen, setChangeUserOpen] = useState(false);
  const [newUserName, setNewUserName] = useState("");
  const [destination, setDestination] = useState<"local" | "flash">("local");
  const upload = useRef<HTMLInputElement>(null);
  const flashDriveConnected = false;
  useEffect(() => {
    if (!state.setupOpen) return;
    const handle = (event: KeyboardEvent) => {
      const keyboardInputOpen = saveAsOpen || changeUserOpen;
      if (keyboardInputOpen) return;
      if (event.key === "Escape") {
        event.preventDefault(); event.stopImmediatePropagation();
        if (saveAsOpen) setSaveAsOpen(false);
        else if (changeUserOpen) setChangeUserOpen(false);
        else if (loadOpen) setLoadOpen(false);
        else if (confirmShutdown) setConfirmShutdown(false);
        else dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
        return;
      }
      if (!keyboardInputOpen) return;
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.key === "Enter") { if (saveAsOpen) void saveAs(); else if (newUserName.trim()) void server.createUser(newUserName.trim()); return; }
      const setValue = saveAsOpen ? setShowName : setNewUserName;
      if (event.key === "Backspace") setValue((value) => value.slice(0, -1));
      else if (event.key.length === 1) setValue((value) => value + event.key);
    };
    window.addEventListener("keydown", handle, true);
    return () => window.removeEventListener("keydown", handle, true);
  }, [state.setupOpen, saveAsOpen, changeUserOpen, loadOpen, confirmShutdown, newUserName]);
  if (!state.setupOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
  const saveLayout = () => void server.saveDeskLayout({ desks: state.desks, activeDeskId: state.activeDeskId });
  async function saveAs() {
    const name = showName.trim();
    if (!name) return;
    if (!await server.saveShowAs(name)) return;
    if (destination === "flash" && server.bootstrap?.active_show) await server.downloadShow({ ...server.bootstrap.active_show, name });
    setSaveAsOpen(false);
    setShowName("");
  }
  const stacked = (content: ReactNode, closeLayer: () => void) => createPortal(
    <div className="stacked-modal-layer" onPointerDown={(event) => { if (event.target === event.currentTarget) closeLayer(); }}>{content}</div>,
    document.body,
  );
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.currentTarget === event.target) close(); }}><section className="modal-card show-modal">
    <button className="modal-close" onClick={close} aria-label="Close">×</button>
    <h2>Show</h2>
    <div className="show-details"><b>{server.bootstrap?.active_show?.name ?? "No active show"}</b><span>Server connected <strong>{server.status === "connected" ? "Yes" : "No"}</strong></span><span>Show revision <strong>{server.bootstrap?.active_show?.revision ?? "—"}</strong></span><span>Operator <strong>{server.session?.user.name ?? "—"}</strong></span></div>
    <div className="show-primary-actions"><button onClick={saveLayout}>Save</button><button onClick={() => setSaveAsOpen(true)}>Save As</button><button onClick={() => setLoadOpen(true)}>Load</button><button onClick={() => setChangeUserOpen(true)}>Change User</button><button onClick={() => dispatch({ type: "SET_MODAL", modal: "debugOpen", value: true })}>Debug</button></div>
    <hr />
    <div className="modal-actions show-navigation"><button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "patch" }); close(); }}>Show Patch</button><button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "setup" }); close(); }}>Enter Setup</button><button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "dmx" }); close(); }}>DMX</button><button className="danger" onClick={() => setConfirmShutdown(true)}>Shut Down Desk</button></div>
    {saveAsOpen && stacked(<div className="nested-modal keyboard-modal" role="dialog" aria-modal="true" aria-label="Save show"><button className="modal-close" onClick={() => setSaveAsOpen(false)}>×</button><h3>Save Show As</h3><div className="save-destination"><button className={destination === "local" ? "active" : ""} onClick={() => setDestination("local")}>This desk</button>{flashDriveConnected && <button className={destination === "flash" ? "active" : ""} onClick={() => setDestination("flash")}>Connected flash drive</button>}</div><input className="show-name-input" autoFocus value={showName} onChange={(event) => setShowName(event.target.value)} placeholder="Show name" aria-label="Show name"/><ModalTextKeyboard value={showName} onChange={setShowName} onEnter={() => void saveAs()} onEscape={() => setSaveAsOpen(false)} actionLabel="Save show"/></div>, () => setSaveAsOpen(false))}
    {loadOpen && stacked(<div className="nested-modal" role="dialog" aria-modal="true" aria-label="Load show"><button className="modal-close" onClick={() => setLoadOpen(false)}>×</button><h3>Load Show</h3><div className="show-library">{server.shows.map((show) => <article key={show.id}><span><b>{show.name}</b><small>Revision {show.revision}</small></span><button onClick={() => { void server.openShow(show.id); setLoadOpen(false); }}>Load</button></article>)}</div><button onClick={() => upload.current?.click()}>Load from flash drive</button><input ref={upload} hidden type="file" accept=".show,application/x-sqlite3" onChange={(event) => { const file = event.target.files?.[0]; if (file) void server.uploadShow(file); event.target.value = ""; }}/></div>, () => setLoadOpen(false))}
    {changeUserOpen && stacked(<div className="nested-modal keyboard-modal" role="dialog" aria-modal="true" aria-label="Change user"><button className="modal-close" onClick={() => setChangeUserOpen(false)}>×</button><h3>Change User</h3><div className="show-library">{server.bootstrap?.users.filter((user) => user.enabled).map((user) => <article key={user.id}><span><b>{user.name}</b><small>{user.id === server.session?.user.id ? "Current user" : "Use this user's programmer"}</small></span><button disabled={user.id === server.session?.user.id} onClick={() => void server.changeUser(user)}>{user.id === server.session?.user.id ? "Logged in" : "Log in"}</button></article>)}</div><div className="user-create-row"><input value={newUserName} onChange={(event) => setNewUserName(event.target.value)} placeholder="New user name" aria-label="New user name"/></div><ModalTextKeyboard value={newUserName} onChange={setNewUserName} onEnter={() => { if (newUserName.trim()) void server.createUser(newUserName.trim()); }} onEscape={() => setChangeUserOpen(false)} actionLabel="Add user"/></div>, () => setChangeUserOpen(false))}
    {confirmShutdown && stacked(<div className="nested-modal shutdown-modal" role="alertdialog" aria-modal="true"><button className="modal-close" onClick={() => setConfirmShutdown(false)}>×</button><h3>Shut Down Desk?</h3><p>Hazardous fixtures will be driven to their safe values before the server stops.</p><div className="modal-actions"><button onClick={() => setConfirmShutdown(false)}>Cancel</button><button className="danger" onClick={() => void server.shutdownServer()}>Shut Down Safely</button></div></div>, () => setConfirmShutdown(false))}
    {server.error && <p className="modal-error">{server.error}</p>}
    <DebugModal />
  </section></div>;
}
