import { Clock } from "./Clock";
import { useApp } from "../../state/AppContext";
import { useRef } from "react";
import type { BuiltInWindow } from "../../types";
import { useServer } from "../../api/ServerContext";
import appIcon from "../../../src-tauri/icons/icon.svg";
import { DeskSettingsModal } from "../modals/DeskSettingsModal";

const builtIns: Array<[BuiltInWindow, string, string]> = [
  ["stage", "⌖", "Stage"], ["fixtures", "♙", "Fixtures"],
  ["presets", "▣", "Presets"], ["playback", "▶", "Playback"], ["dynamics", "∿", "Dynamics"],
  ["channels", "▥", "Channels"], ["dmx", "⠿", "DMX"],
];

export function LeftDock() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const longPress = useRef<number | null>(null);
  const held = useRef(false);
  return <aside className="left-dock">
    <button className="dock-identity" aria-label="Open show menu" onClick={() => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: true })}><span className="app-mark"><img src={appIcon} alt="" /></span><Clock/><b><span className={server.showDirty ? "show-dirty-dot" : "connection-dot"}>●</span> {server.bootstrap?.active_show?.name ?? "Show"}</b></button>
    <button className={`dock-section-key ${state.dockMode === "desks" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "desks" })}>DESKS</button>
    {state.dockMode === "desks" && <div className="dock-list dock-list-enter">{state.desks.map((desk) => <button key={desk.id} className={`dock-entry ${state.activeDeskId === desk.id ? "active" : ""}`} onPointerDown={() => { held.current = false; longPress.current = window.setTimeout(() => { held.current = true; dispatch({ type: "OPEN_DESK_SETTINGS", id: desk.id }); }, 650); }} onPointerUp={() => { if (longPress.current) window.clearTimeout(longPress.current); }} onPointerCancel={() => { if (longPress.current) window.clearTimeout(longPress.current); }} onClick={() => { if (!held.current) dispatch({ type: "OPEN_DESK", id: desk.id }); held.current = false; }}><span>{desk.icon ?? "⊞"}</span>{desk.name}</button>)}<button className="dock-entry" onClick={() => dispatch({ type: "NEW_DESK" })}><span>＋</span>New desk</button></div>}
    <button className={`dock-section-key builtins-key ${state.dockMode === "builtins" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "builtins" })}>BUILT-INS</button>
    {state.dockMode === "builtins" && <div className="dock-list builtins-list dock-list-enter">{builtIns.map(([kind, icon, label]) => <button key={kind} className={`dock-entry ${state.builtIn === kind ? "active" : ""}`} onClick={() => dispatch({ type: "OPEN_BUILTIN", kind })}><span>{icon}</span>{label}</button>)}</div>}
    <DeskSettingsModal />
  </aside>;
}
