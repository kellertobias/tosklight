import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { programmerValueCount } from "./programmerActivity";
import { Button } from "../common";
import { editCommandWithSoftwareKey, softwareKeypadRows, type SoftwareKey } from "./softwareKeypad";

const keys = softwareKeypadRows.flat();
const labels: Partial<Record<SoftwareKey, string>> = { BACKSPACE: "←", ENT: "ENT" };

export function NumericPad() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [clearStage, setClearStage] = useState(0);
  const ownProgrammer = server.bootstrap?.active_programmers.find((programmer) => programmer.user_id === server.session?.user.id);
  const hasClearContent = server.selectedFixtures.length > 0 || programmerValueCount(ownProgrammer) > 0;
  const clearClass = clearStage === 2 ? "clear-warning" : hasClearContent ? "clear-active" : "clear-idle";
  useEffect(() => { if (server.selectedFixtures.length) setClearStage(0); }, [server.selectedFixtures]);
  const press = (key: SoftwareKey) => {
    if (key === "CLR") {
      if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
      if (state.cueListSetArmed) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
      server.setCommandLine("");
      if (state.preload !== "idle") { void server.preloadAction("clear"); return; }
      if (clearStage === 0 && !hasClearContent) return;
      if (clearStage === 0) { void server.setSelection([]); setClearStage(1); }
      else if (clearStage === 1) { void server.clearProgrammerValues(); setClearStage(2); }
      else { if (server.session) void server.clearProgrammer(server.session.session_id); setClearStage(0); }
      return;
    }
    if (key === "SET" && state.builtIn === "patch") return dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed });
    if (key === "SET" && document.querySelector(".cuelist-window.pool-window")) {
      if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
      return dispatch({ type: "SET_CUELIST_SET_ARMED", value: !state.cueListSetArmed });
    }
    if (key === "SET" && (state.builtIn === "presets" || state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "presets"))) return dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed });
    if (key === "UND") { setClearStage(0); return void server.undoProgrammer(); }
    if (key === "ENT") {
      setClearStage(0);
      return void server.executeCommandLine().then((ok) => { if (ok && state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false }); });
    }
    const edited = editCommandWithSoftwareKey(server.commandLine, key);
    server.setCommandLine(edited.command);
    if (edited.execute) void server.executeCommandLine(edited.command);
    setClearStage(0);
  };
  return <div className="numeric-pad">{keys.map((key) => <Button
    onClick={() => press(key)}
    data-keypad-key={key}
    className={`${["AT", "TRU", "GRP", "SET", "DIV", "CUE", "UND", "DEL", "MOV", "CPY", "+", "CLR"].includes(key) ? "action" : key === "ENT" ? "enter" : ""} ${key === "SET" && ((state.builtIn === "patch" && state.patchSetArmed) || state.presetSetArmed || state.cueListSetArmed) ? "patch-set-armed" : key === "CLR" ? `clear ${clearClass}` : ""}`}
    key={key}
  >{labels[key] ?? key}</Button>)}</div>;
}
