import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { programmerValueCount } from "./programmerActivity";
import { Button, Input } from "../common";
import { editTargetedCommandWithSoftwareKey, softwareKeyFromKeyboard } from "./softwareKeypad";
import { openUpdateSettings, openUpdateTargetMenu } from "./updateWorkflow";
import { canAdvancePlaybackPage } from "./PlaybackPageDialogs";
import "./CommandLineHistory.css";

export function CommandLineBar() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const [completed, setCompleted] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [persistentError, setPersistentError] = useState<string | null>(null);
  const [errorOpen, setErrorOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const historyPanel = useRef<HTMLElement | null>(null);
  const storeHold = useRef<number | null>(null);
  const storeHeld = useRef(false);
  const storeSuppressUntil = useRef(0);
  const recordPressMode = useRef<"record" | "update" | null>(null);
  const updateKeyboardHold = useRef<number | null>(null);
  const updateKeyboardActive = useRef(false);
  const updateKeyboardHeld = useRef(false);
  const preloadHold = useRef<number | null>(null);
  const preloadHeld = useRef(false);
  const keyboardFlash = useRef(new Map<string, number>());
  useEffect(() => { if (server.error) setPersistentError(server.error); }, [server.error]);
  useEffect(() => {
    if (commandError && server.error) setCommandError(server.error);
  }, [server.error, commandError]);
  useEffect(() => {
    const showCommandError = (event: Event) => {
      const message = (event as CustomEvent<string>).detail;
      setCompleted(false);
      setCommandError(message || "The command could not be executed.");
    };
    window.addEventListener("light:command-error", showCommandError);
    return () => window.removeEventListener("light:command-error", showCommandError);
  }, []);
  useEffect(() => {
    if (!historyOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setHistoryOpen(false);
    };
    const closeOutside = (event: PointerEvent) => {
      if (historyPanel.current?.contains(event.target as Node)) return;
      if ((event.target as Element | null)?.closest(".command-input")) return;
      setHistoryOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape, true);
    window.addEventListener("pointerdown", closeOutside, true);
    return () => {
      window.removeEventListener("keydown", closeOnEscape, true);
      window.removeEventListener("pointerdown", closeOutside, true);
    };
  }, [historyOpen]);
  const playback = state.controlMode === "playbacks";
  const ownProgrammer = server.bootstrap?.active_programmers.find((programmer) => programmer.session_id === server.session?.session_id);
  const hasRecordableContent = server.selectedFixtures.length > 0 || programmerValueCount(ownProgrammer) > 0 || state.preload !== "idle" || state.preloadActive;
  const pendingProgrammerCount = (ownProgrammer?.preload_pending?.length ?? 0)
    + Object.values(ownProgrammer?.preload_group_pending ?? {}).reduce((count, attributes) => count + Object.keys(attributes).length, 0);
  const pendingPlaybackLabels = (ownProgrammer?.preload_playback_pending ?? []).map((pending) => `${pending.action.replaceAll("-", " ").toUpperCase()} ${pending.playback_number}`);
  const pendingSummary = [pendingProgrammerCount ? `PROG ${pendingProgrammerCount}` : "", ...pendingPlaybackLabels].filter(Boolean).join(" · ");
  const preloadLabel = state.preload === "blind" ? "PRELOAD GO" : "PRELOAD";
  const advancePreload = async () => {
    await server.preloadAction(state.preload === "blind" ? "go" : "enter");
    dispatch({ type: "ADVANCE_PRELOAD" });
  };
  const releasePreload = async () => {
    await server.preloadAction("release");
    dispatch({ type: "RELEASE_PRELOAD" });
  };
  const execute = async () => {
    const ok = await server.executeCommandLine();
    setCompleted(ok);
    if (ok && state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
    if (ok && state.updateArmed) dispatch({ type: "SET_UPDATE_ARMED", value: false });
    if (!ok) setCommandError(server.error ?? "The command could not be executed.");
  };
  const replaceCommand = (value: string, pristine = false) => {
    setCompleted(false);
    setCommandError(null);
    server.setCommandLine(value, pristine);
  };
  const toggleRecord = () => {
    const armed = !state.storeArmed;
    if (armed && state.cueListSetArmed) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
    dispatch({ type: "SET_STORE_ARMED", value: armed });
    if (armed) replaceCommand("RECORD ");
    else if (/^RECORD\b/i.test(server.commandLine)) replaceCommand(server.commandLine.replace(/^RECORD\s*/i, ""));
  };
  const armUpdateOrMenu = () => {
    if (state.updateArmed) {
      openUpdateTargetMenu();
      return;
    }
    if (state.cueListSetArmed) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
    if (state.playbackSetArmed) dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
    if (state.presetSetArmed) dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
    dispatch({ type: "SET_UPDATE_ARMED", value: true });
    replaceCommand("UPDATE ");
  };
  const beginRecordPress = (shifted: boolean) => {
    storeHeld.current = false;
    recordPressMode.current = shifted ? "update" : "record";
    storeHold.current = window.setTimeout(() => {
      storeHeld.current = true;
      storeSuppressUntil.current = performance.now() + 1000;
      if (recordPressMode.current === "update") openUpdateSettings();
      else dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: true });
    }, 650);
  };
  const endRecordPress = () => {
    if (storeHold.current) window.clearTimeout(storeHold.current);
    storeHold.current = null;
  };
  const completeRecordPress = (shifted: boolean) => {
    const updateGesture = recordPressMode.current === "update" || shifted;
    recordPressMode.current = null;
    if (storeHeld.current || performance.now() < storeSuppressUntil.current) {
      storeHeld.current = false;
      return;
    }
    if (updateGesture) armUpdateOrMenu();
    else toggleRecord();
  };
  useEffect(() => {
	if (hardware || !state.regularNumberShortcuts) return;
    const openRunningMenu = (event: KeyboardEvent) => {
      if (event.code !== "Delete" || !event.shiftKey || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target instanceof HTMLElement ? event.target : null;
      if (target?.closest("input,textarea,select,[contenteditable=true]") && !target.closest(".command-input")) return;
      event.preventDefault();
      dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
    };
    window.addEventListener("keydown", openRunningMenu);
    return () => window.removeEventListener("keydown", openRunningMenu);
  }, [dispatch, hardware, state.regularNumberShortcuts]);
  useEffect(() => {
    if (hardware || !state.regularNumberShortcuts) return;
    const triggerPlaybackButton = (event: KeyboardEvent, slot: number) => {
      const page = server.playbacks?.pages.find((candidate) => candidate.number === server.playbacks?.active_page);
      const playbackNumber = page?.slots[String(slot)];
      const definition = server.playbacks?.pool.find((candidate) => candidate.number === playbackNumber);
      const action = definition?.buttons[0];
      if (!definition || !action || action === "none") return;
      if (action === "flash") {
        if (event.repeat) return;
        keyboardFlash.current.set(event.code, definition.number);
        void server.poolPlaybackAction(definition.number, "flash", { pressed: true });
      } else {
        void server.poolPlaybackAction(definition.number, action.replaceAll("_", "-") as Parameters<typeof server.poolPlaybackAction>[1]);
      }
    };
    const keydown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const target = event.target instanceof HTMLElement ? event.target : null;
      const commandInput = Boolean(target?.closest(".command-input"));
      if (!commandInput && target?.closest("input,textarea,select,[contenteditable=true]")) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (/^F(?:[1-9]|1[0-3])$/.test(event.key)) {
        event.preventDefault();
        const number = Number(event.key.slice(1));
        if (number <= 8) triggerPlaybackButton(event, number);
        else {
          const group = String.fromCharCode(65 + number - 9) as "A" | "B" | "C" | "D" | "E";
          dispatch({ type: "SET_SPEED_GROUP", value: group });
          window.dispatchEvent(new CustomEvent("light:speed-group-tap", { detail: group }));
        }
        return;
      }
      if (event.code === "PageUp" || event.code === "PageDown") {
        event.preventDefault();
        const current = server.playbacks?.active_page ?? state.playbackPage + 1;
        const pages = server.playbacks?.pages ?? [];
        const page = current + (event.code === "PageUp" ? 1 : -1);
        if (page < 1) return;
        if (pages.some((item) => item.number === page)) {
          dispatch({ type: "SET_PLAYBACK_PAGE", page: page - 1 });
          void server.setPlaybackPage(page);
        } else if (event.code === "PageUp" && canAdvancePlaybackPage(pages, current)) {
          void server.savePlaybackPage({ number: page, name: `Page ${page}`, slots: {} }).then((saved) => {
            if (!saved) return;
            dispatch({ type: "SET_PLAYBACK_PAGE", page: page - 1 });
            void server.setPlaybackPage(page);
          });
        }
        return;
      }
      const key = softwareKeyFromKeyboard(event, true);
      if (!key) return;
      if (key === "ESC") {
        event.preventDefault();
        if (document.querySelector("[role=dialog],.stacked-modal-layer")) return;
        if (state.updateArmed) {
          dispatch({ type: "SET_UPDATE_ARMED", value: false });
          dispatch({ type: "SET_SHIFT_ARMED", value: false });
          replaceCommand("", true);
        }
        else if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
        else if (state.cueListSetArmed) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
        else if (persistentError) {
          setPersistentError(null);
          setErrorOpen(false);
          server.dismissError();
        } else replaceCommand("", true);
        return;
      }
      event.preventDefault();
      if (key === "SET" && (completed || server.commandLinePristine) && state.builtIn === "patch") {
        document.querySelector<HTMLButtonElement>('[data-keypad-key="SET"]')?.click();
      }
      else if (key === "REC" && event.shiftKey) {
        if (event.repeat || updateKeyboardActive.current) return;
        updateKeyboardActive.current = true;
        updateKeyboardHeld.current = false;
        updateKeyboardHold.current = window.setTimeout(() => {
          updateKeyboardHeld.current = true;
          openUpdateSettings();
        }, 650);
      }
      else if (key === "REC") document.querySelector<HTMLButtonElement>(".global-store-button")?.click();
      else if (key === "PRE") document.querySelector<HTMLButtonElement>(".preload-button")?.click();
      else if (key === "CLR" || key === "UND") document.querySelector<HTMLButtonElement>(`[data-keypad-key="${key}"]`)?.click();
      else if (key === "ENT") void execute();
      else {
        const edited = editTargetedCommandWithSoftwareKey(
          completed ? server.commandTargetMode : server.commandLine,
          key,
          server.commandTargetMode,
          completed || server.commandLinePristine,
        );
        replaceCommand(edited.command, edited.pristine);
        if (edited.execute) void server.executeCommandLine(edited.command);
      }
    };
    const keyup = (event: KeyboardEvent) => {
      if (event.code === "End" && updateKeyboardActive.current) {
        if (updateKeyboardHold.current) window.clearTimeout(updateKeyboardHold.current);
        updateKeyboardHold.current = null;
        updateKeyboardActive.current = false;
        if (!updateKeyboardHeld.current) armUpdateOrMenu();
        updateKeyboardHeld.current = false;
        return;
      }
      const playbackNumber = keyboardFlash.current.get(event.code);
      if (playbackNumber == null) return;
      keyboardFlash.current.delete(event.code);
      void server.poolPlaybackAction(playbackNumber, "flash", { pressed: false });
    };
    window.addEventListener("keydown", keydown);
    window.addEventListener("keyup", keyup);
    return () => {
      window.removeEventListener("keydown", keydown);
      window.removeEventListener("keyup", keyup);
      if (updateKeyboardHold.current) window.clearTimeout(updateKeyboardHold.current);
      updateKeyboardHold.current = null;
      updateKeyboardActive.current = false;
      for (const playbackNumber of keyboardFlash.current.values()) void server.poolPlaybackAction(playbackNumber, "flash", { pressed: false });
      keyboardFlash.current.clear();
    };
  }, [hardware, completed, persistentError, state.storeArmed, state.updateArmed, state.cueListSetArmed, state.playbackSetArmed, state.presetSetArmed, state.regularNumberShortcuts, state.playbackPage, server.playbacks, server.commandLine, server.commandTargetMode, server.commandLinePristine, server.poolPlaybackAction, server.savePlaybackPage, server.setPlaybackPage]);
  return (
    <header
      className={`command-line-bar command-line-left ${playback ? "playback-mode" : ""} ${commandError ? "has-command-error" : ""}`}
    >
      {commandError && <div className="command-error-message" role="alert"><span>{commandError}</span><Button onClick={() => { setCommandError(null); server.dismissError(); }}>Acknowledge</Button></div>}
      <Button
        className={`mode-toggle ${playback ? "playbacks-active" : ""}`}
        onClick={() => dispatch({ type: "TOGGLE_CONTROL_MODE" })}
      >
        <span className="mode-icon">{playback ? "▶" : "⌨"}</span>
        <span>
          <b>PROG.</b>
          <small>PLAYBK</small>
        </span>
      </Button>
      <div className="command-field"><Input
        className={`command-input ${state.preload === "blind" ? "blind" : ""} ${state.updateArmed ? "update-armed" : ""} ${completed ? "completed" : ""} ${commandError ? "error" : ""}`}
        aria-label="Command line"
        value={server.commandLine}
        placeholder=""
        onClick={() => setHistoryOpen(true)}
        onChange={(event) =>
          replaceCommand(
            completed ? `${server.commandTargetMode} ${event.target.value.slice(-1)}` : event.target.value,
          )
        }
        onKeyDown={(event) => {
          if (event.key === "Enter") { event.stopPropagation(); void execute(); }
        }}
      />{!hardware && <Button className="command-escape" onClick={() => replaceCommand("", true)}>ESC</Button>}
        <Button aria-label={`DMX ${server.bootstrap?.frame_rate_hz ?? "—"}Hz; ${server.bootstrap?.active_timecode ?? "No Timecode"}. Open running and output controls`} className={`command-status ${server.status}`} title="Open running and output controls" onClick={() => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true })}>
          <span className={state.blackout ? "blackout-status" : ""}>{state.blackout ? <><i><span className="status-label-full">DMX </span>{server.bootstrap?.frame_rate_hz ?? "—"}Hz</i><b>BLACKOUT</b></> : <><span className="status-label-full">DMX {server.bootstrap?.frame_rate_hz ?? "—"}Hz</span><span className="status-label-compact">{server.bootstrap?.frame_rate_hz ?? "—"}Hz</span></>}</span>
          <span className={`timecode-status ${server.bootstrap?.active_timecode ? "timecode-active" : "timecode-idle"}`}>{server.bootstrap?.active_timecode ?? <><span className="status-label-full">No Timecode</span><span className="status-label-compact">No TC</span></>}</span>
        </Button>
      </div>
      {completed && (
        <span className="command-complete" aria-label="Command applied">
          ✓
        </span>
      )}
      {historyOpen && createPortal(
        <section className="command-history-panel" role="dialog" aria-modal="false" aria-label="Command line history" ref={historyPanel}>
          <header>
            <div><h2>Command Line History</h2><small>Newest first · this desk · last 50 results</small></div>
            <Button aria-label="Close command line history" onClick={() => setHistoryOpen(false)}>×</Button>
          </header>
          <div className="command-history-list">
            {server.commandHistory.length === 0
              ? <p className="command-history-empty">No accepted or rejected commands yet.</p>
              : server.commandHistory.map((entry) => <article className={`command-history-entry ${entry.status}`} key={entry.id}>
                <div className="command-history-entry-main">
                  <span className="command-history-status">{entry.status === "accepted" ? "Accepted" : "Rejected"}</span>
                  <code>{entry.command}</code>
                  <small>{new Date(entry.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })} · {entry.source === "osc" ? "attached hardware" : "desk"}</small>
                </div>
                <p>{entry.feedback}</p>
                <Button onClick={() => { replaceCommand(entry.command); setHistoryOpen(false); }}>Reuse</Button>
              </article>)}
          </div>
        </section>,
        document.body,
      )}
      {errorOpen && persistentError && <div className="persistent-error-popover" role="alertdialog"><header><b><span>▲</span> Desk error</b><Button onClick={() => setErrorOpen(false)}>×</Button></header><pre>{persistentError}</pre><Button onClick={() => { setPersistentError(null); server.dismissError(); setErrorOpen(false); }}>Acknowledge</Button></div>}
      <div className="command-record-preload">
        <Button
          className={`global-store-button ${state.updateArmed ? "update-armed" : state.storeArmed ? "armed" : hasRecordableContent ? "record-ready" : "record-empty"}`}
          aria-pressed={state.updateArmed || state.storeArmed}
          title="REC · Shift+REC: Update · hold Shift+REC: Update Settings"
          onPointerDown={(event) => beginRecordPress(state.shiftArmed || event.shiftKey)}
          onPointerUp={endRecordPress}
          onPointerCancel={() => { endRecordPress(); recordPressMode.current = null; }}
          onClick={(event) => completeRecordPress(state.shiftArmed || event.shiftKey)}
        >{state.updateArmed ? "UPDATE ARMED" : state.storeArmed ? "REC ARMED" : "REC"}</Button>
        <Button
          className={`preload-button ${state.preload === "blind" ? "preload-go" : "preload-enter"}`}
          title={state.preload === "blind" && pendingSummary ? `Pending Preload: ${pendingSummary}` : state.preloadActive ? "Hold to release the active preload scene" : undefined}
          onPointerDown={() => {
            preloadHeld.current = false;
            if (!state.preloadActive) return;
            preloadHold.current = window.setTimeout(() => {
              preloadHeld.current = true;
              void releasePreload();
            }, 650);
          }}
          onPointerUp={() => { if (preloadHold.current !== null) window.clearTimeout(preloadHold.current); preloadHold.current = null; }}
          onPointerCancel={() => { if (preloadHold.current !== null) window.clearTimeout(preloadHold.current); preloadHold.current = null; }}
          onContextMenu={(event) => event.preventDefault()}
          onClick={() => { if (!preloadHeld.current) void advancePreload(); preloadHeld.current = false; }}
        >
          <b>{preloadLabel}</b>
          {state.preload === "blind" && pendingSummary
            ? <small aria-label={`Pending Preload: ${pendingSummary}`}>{pendingSummary}</small>
            : state.preloadActive && <small>(Hold: release)</small>}
        </Button>
      </div>
    </header>
  );
}
