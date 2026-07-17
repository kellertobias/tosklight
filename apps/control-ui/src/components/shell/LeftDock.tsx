import { Clock } from "./Clock";
import { useApp } from "../../state/AppContext";
import { useRef } from "react";
import type { BuiltInWindow } from "../../types";
import { useServer } from "../../api/ServerContext";
import appIcon from "../../../src-tauri/icons/icon.svg";
import { DeskSettingsModal } from "../modals/DeskSettingsModal";
import { Button } from "../common";
import { getShowIndicator } from "./showIndicator";

export const builtIns: Array<[BuiltInWindow, string, string]> = [
  ["stage", "⌖", "Stage"], ["fixtures", "♙", "Fixtures"],
  ["presets", "▣", "Presets"], ["cuelists", "▶", "Cuelists"], ["dynamics", "∿", "Dynamics"],
  ["channels", "▥", "Channels"], ["dmx", "⠿", "DMX"],
];

export function LeftDock() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const longPress = useRef<number | null>(null);
  const held = useRef(false);
  const suppressUntil = useRef(0);
  const showIndicator = getShowIndicator(server.status);
  const activeShow = server.bootstrap?.active_show;
  const showIdentity = activeShow?.revision_copy ? `Revision Copy · ${activeShow.name}` : activeShow?.name ?? "Show";
  const identityDetail = activeShow?.revision_copy
    ? `${showIdentity}. Source: ${activeShow.revision_copy.show_name}, Revision ${activeShow.revision_copy.revision} · ${activeShow.revision_copy.revision_name}. ${showIndicator.detail}`
    : `${showIndicator.label}. ${showIndicator.detail}`;
  return <aside className="left-dock">
    <Button className={`dock-identity ${activeShow?.revision_copy ? "revision-copy-active" : ""}`} aria-label={`Open show menu. ${showIdentity}. ${showIndicator.label}`} title={identityDetail} onClick={() => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: true })}><span className="app-mark"><img src={appIcon} alt="" /></span><Clock/><b><span className={`show-status-dot ${showIndicator.className}`} aria-hidden="true">●</span> {showIdentity}</b></Button>
    <Button className={`dock-section-key ${state.dockMode === "desks" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "desks" })}>DESKTOPS</Button>
    {state.dockMode === "desks" && <div className="dock-list dock-list-enter">{state.desks.map((desk) => <Button key={desk.id} className={`dock-entry ${state.activeDeskId === desk.id ? "active" : ""}`} onPointerDown={() => { held.current = false; longPress.current = window.setTimeout(() => { held.current = true; suppressUntil.current = performance.now() + 1000; dispatch({ type: "OPEN_DESK_SETTINGS", id: desk.id }); }, 650); }} onPointerUp={() => { if (longPress.current) window.clearTimeout(longPress.current); longPress.current = null; }} onPointerCancel={() => { if (longPress.current) window.clearTimeout(longPress.current); longPress.current = null; }} onClick={() => { if (!held.current && performance.now() >= suppressUntil.current) dispatch({ type: "OPEN_DESK", id: desk.id }); held.current = false; }}><span>{desk.icon ?? "⊞"}</span>{desk.name}</Button>)}<Button className="dock-entry" onClick={() => dispatch({ type: "NEW_DESK" })}><span>＋</span>New desktop</Button></div>}
    <Button className={`dock-section-key builtins-key ${state.dockMode === "builtins" ? "active" : ""}`} onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "builtins" })}>BUILT-INS</Button>
    {state.dockMode === "builtins" && <div className="dock-list builtins-list dock-list-enter">{builtIns.map(([kind, icon, label]) => <Button key={kind} className={`dock-entry ${state.builtIn === kind ? "active" : ""}`} onClick={() => dispatch({ type: "OPEN_BUILTIN", kind })}><span>{icon}</span>{label}</Button>)}</div>}
    <DeskSettingsModal />
  </aside>;
}
