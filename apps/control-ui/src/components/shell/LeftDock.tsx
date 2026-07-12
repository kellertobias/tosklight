import { Clock } from "./Clock";
import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";
import { useServer } from "../../api/ServerContext";

const builtIns: Array<[BuiltInWindow, string, string]> = [
  ["stage", "⌖", "Stage"], ["groups", "◉", "Groups"], ["fixtures", "♙", "Fixtures"],
  ["presets", "▣", "Presets"], ["playback", "▶", "Playback"], ["dynamics", "∿", "Dynamics"],
  ["channels", "▥", "Channels"], ["dmx", "⠿", "DMX"], ["setup", "⚙", "Setup"],
];

export function LeftDock() {
  const { state, dispatch } = useApp();
  const server = useServer();
  return <aside className="left-dock">
    <div className="app-mark" aria-label="Light application">L</div>
    <button className={`dock-section-key ${state.dockMode === "desks" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "desks" })}>DESKS</button>
    {state.dockMode === "desks" && <div className="dock-list dock-list-enter">{state.desks.map((desk) => <button key={desk.id} className={`dock-entry ${state.activeDeskId === desk.id ? "active" : ""}`} onClick={() => dispatch({ type: "OPEN_DESK", id: desk.id })}><span>⊞</span>{desk.name}</button>)}<button className="dock-entry" onClick={() => dispatch({ type: "NEW_DESK" })}><span>＋</span>New desk</button></div>}
    <button className={`dock-section-key builtins-key ${state.dockMode === "builtins" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "builtins" })}>BUILT-INS</button>
    {state.dockMode === "builtins" && <div className="dock-list builtins-list dock-list-enter">{builtIns.map(([kind, icon, label]) => <button key={kind} className={`dock-entry ${state.builtIn === kind ? "active" : ""}`} onClick={() => dispatch({ type: "OPEN_BUILTIN", kind })}><span>{icon}</span>{label}</button>)}</div>}
    <div className="dock-show"><Clock/><button className="show-setup-button" onClick={() => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: true })}><span className={server.status === "connected" ? "changed" : "connection-dot"}>●</span><b>{server.bootstrap?.active_show?.name ?? "Show"}</b></button></div>
  </aside>;
}
