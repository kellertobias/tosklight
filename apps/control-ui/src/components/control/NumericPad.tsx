import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { programmerValueCount } from "./programmerActivity";
import { Button } from "../common";
import { editTargetedCommandWithSoftwareKey, type SoftwareKey } from "./softwareKeypad";
import type { BuiltInWindow } from "../../types";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";
import { numericPadLayout, softwareKeyLabel } from "../../../../shared/programmerKeypad";
import { HighlightControls } from "./HighlightControls";

export { numericPadLayout } from "../../../../shared/programmerKeypad";
const shiftedWindows: Partial<Record<SoftwareKey, BuiltInWindow>> = {
  ".": "help",
  "0": "fixtures",
  "1": "groups",
  "3": "cuelists",
  "5": "dynamics",
  "6": "channels",
};

export function NumericPad() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const ownProgrammer = server.bootstrap?.active_programmers.find((programmer) => programmer.session_id === server.session?.session_id);
  const hasSelection = server.selectedFixtures.length > 0;
  const hasProgrammerValues = programmerValueCount(ownProgrammer) > 0;
  const clearClass = hasSelection ? "clear-active" : hasProgrammerValues ? "clear-warning" : "clear-idle";
  const press = (key: SoftwareKey) => {
    if (key === "SHIFT") { dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed }); return; }
    if (state.shiftArmed) {
      dispatch({ type: "SET_SHIFT_ARMED", value: false });
      if (key === "TIME") {
        const current = server.commandLine.trim();
        const command = server.commandLinePristine || current === "FIXTURE" || current === "GROUP"
          ? "SPD GRP"
          : `${current} SPD GRP`;
        server.setCommandLine(command, false);
        return;
      }
      if (key === "CLR" || key === "DEL") {
        dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
        return;
      }
      if (key === "2") {
        dispatch({ type: "SET_PRESET_FAMILY", family: "All" });
        dispatch({ type: "OPEN_BUILTIN", kind: "presets" });
        return;
      }
      if (key === "4") {
        const activePlayback = server.playbacks?.selected_playback;
        dispatch({ type: "OPEN_BUILTIN", kind: "cuelists" });
        if (activePlayback != null) dispatch({ type: "OPEN_BUILTIN_CUELIST", number: activePlayback });
        return;
      }
      if (key === "7" || key === "8" || key === "9") {
        const desk = state.desks[Number(key) - 7];
        if (desk) dispatch({ type: "OPEN_DESK", id: desk.id });
        return;
      }
      const kind = shiftedWindows[key];
      if (kind) { dispatch({ type: "OPEN_BUILTIN", kind }); return; }
    }
    if (key === "CLR") {
      if (state.updateArmed) dispatch({ type: "SET_UPDATE_ARMED", value: false });
      if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
      if (state.cueListSetArmed) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
      if (state.playbackSetArmed) dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
      server.resetCommandLine();
      if (state.preload !== "idle") { void server.preloadAction("clear"); return; }
      if (hasSelection) void server.setSelection([]);
      else if (hasProgrammerValues) void server.clearProgrammerValues();
      return;
    }
    if (key === "SET" && server.commandLinePristine && state.builtIn === "patch") return dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed });
    if (key === "SET" && server.commandLinePristine && document.querySelector(".cuelist-window.pool-window")) {
      if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
      return dispatch({ type: "SET_CUELIST_SET_ARMED", value: !state.cueListSetArmed });
    }
    if (key === "SET" && server.commandLinePristine && document.querySelector(".playback-fader-bank,.virtual-playback-grid")) return dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: !state.playbackSetArmed });
    if (key === "SET" && server.commandLinePristine && (state.builtIn === "presets" || (state.builtIn == null && state.desks.find((desk) => desk.id === state.activeDeskId)?.panes.some((pane) => pane.kind === "presets")))) return dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed });
    if (key === "UND") return void server.undoProgrammer();
    if (key === "ENT") {
      return void server.executeCommandLine().then((ok) => {
        if (ok && state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
        if (ok && state.updateArmed) dispatch({ type: "SET_UPDATE_ARMED", value: false });
      });
    }
    const edited = editTargetedCommandWithSoftwareKey(server.commandLine, key, server.commandTargetMode, server.commandLinePristine);
    server.setCommandLine(edited.command, edited.pristine);
    if (edited.execute) void server.executeCommandLine(edited.command);
  };
  const renderKeys = (section: "commands" | "numbers") => numericPadLayout.filter((item) => item.section === section).map(({ key, column, row, rowSpan = 1 }) => {
    const sectionColumn = section === "commands" ? column : column - 3;
    const displayRow = row + (section === "numbers" ? 1 : 0);
    return <Button
    onClick={() => press(key)}
    data-keypad-key={key}
    data-grid-column={sectionColumn}
    data-grid-row={displayRow}
    style={{ gridColumn: sectionColumn, gridRow: `${displayRow} / span ${rowSpan}` }}
    className={`${["AT", "TRU", "GRP", "SET", "DIV", "CUE", "UND", "DEL", "MOV", "CPY", "+", "-", "TIME", "SHIFT", "CLR"].includes(key) ? "action" : key === "ENT" ? "enter" : ""} ${key === "SHIFT" && state.shiftArmed ? "shift-armed" : ""} ${key === "SET" && ((state.builtIn === "patch" && state.patchSetArmed) || state.presetSetArmed || state.cueListSetArmed || state.playbackSetArmed) ? "patch-set-armed" : key === "CLR" ? `clear ${clearClass}` : ""}`}
    key={key}
  >{softwareKeyLabel(key)}</Button>;
  });
  return <div className="numeric-pad programmer-number-block">
    <div className="numeric-pad-section numeric-pad-command-section">
      <div className="numeric-pad-fade" style={{ gridColumn: "1 / span 2", gridRow: 1 }}><ProgrammerFadeFader compact/></div>
      {renderKeys("commands")}
    </div>
    <div className="numeric-pad-section numeric-pad-number-section"><HighlightControls/>{renderKeys("numbers")}</div>
  </div>;
}
