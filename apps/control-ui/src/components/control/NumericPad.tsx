import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { programmerValueCount } from "./programmerActivity";
import { Button } from "../common";
import { removeCommandToken } from "./commandLineEditing";

const keys = ["GRP", "SET", "DIV", "CLEAR", "7", "8", "9", "AT", "4", "5", "6", "FULL", "1", "2", "3", "THRU", ".", "0", "←", "ENTER"];

export function NumericPad() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [clearStage, setClearStage] = useState(0);
  const ownProgrammer = server.bootstrap?.active_programmers.find((programmer) => programmer.user_id === server.session?.user.id);
  const hasClearContent = server.selectedFixtures.length > 0 || programmerValueCount(ownProgrammer) > 0;
  const clearClass = clearStage === 2 ? "clear-warning" : hasClearContent ? "clear-active" : "clear-idle";
  useEffect(() => { if (server.selectedFixtures.length) setClearStage(0); }, [server.selectedFixtures]);
  const press = (key: string) => {
    if (key === "←") return server.setCommandLine(removeCommandToken(server.commandLine));
    if (key === "CLEAR") {
      if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
      server.setCommandLine("");
      if (state.preload !== "idle") { void server.preloadAction("clear"); return; }
      if (clearStage === 0 && !hasClearContent) return;
      if (clearStage === 0) { void server.setSelection([]); setClearStage(1); }
      else if (clearStage === 1) { void server.clearProgrammerValues(); setClearStage(2); }
      else { if (server.session) void server.clearProgrammer(server.session.session_id); setClearStage(0); }
      return;
    }
    if (key === "SET" && state.builtIn === "patch") return dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed });
    if (key === "SET" && (state.builtIn === "presets" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "presets"))) return dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed });
    if (key === "ENTER") { setClearStage(0); return void server.executeCommandLine(); }
    const commandKey = key === "GRP" ? "GROUP" : key;
    const token = ["AT", "FULL", "THRU", "GROUP", "SET", "DIV"].includes(commandKey) ? ` ${commandKey} ` : commandKey;
    server.setCommandLine(`${server.commandLine}${token}`.replace(/\s+/g, " ").trimStart());
    setClearStage(0);
  };
  return <div className="numeric-pad">{keys.map((key) => <Button
    onClick={() => press(key)}
    className={`${["AT", "FULL", "THRU", "GRP", "SET", "DIV", "CLEAR"].includes(key) ? "action" : key === "ENTER" ? "enter" : ""} ${key === "SET" && ((state.builtIn === "patch" && state.patchSetArmed) || state.presetSetArmed) ? "patch-set-armed" : key === "CLEAR" ? `clear ${clearClass}` : ""}`}
    key={key}
  >{key}</Button>)}</div>;
}
