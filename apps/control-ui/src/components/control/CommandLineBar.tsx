import { useEffect, useRef, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { programmerValueCount } from "./programmerActivity";
import { Button, Input } from "../common";
import { removeCommandToken } from "./commandLineEditing";

export function CommandLineBar() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [completed, setCompleted] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [persistentError, setPersistentError] = useState<string | null>(null);
  const [errorOpen, setErrorOpen] = useState(false);
  const storeHold = useRef<number | null>(null);
  const storeHeld = useRef(false);
  const storeSuppressUntil = useRef(0);
  const preloadHold = useRef<number | null>(null);
  const preloadHeld = useRef(false);
  useEffect(() => { if (server.error) setPersistentError(server.error); }, [server.error]);
  useEffect(() => {
    if (commandError && server.error) setCommandError(server.error);
  }, [server.error, commandError]);
  const playback = state.controlMode === "playbacks";
  const ownProgrammer = server.bootstrap?.active_programmers.find((programmer) => programmer.user_id === server.session?.user.id);
  const hasRecordableContent = server.selectedFixtures.length > 0 || programmerValueCount(ownProgrammer) > 0 || state.preload !== "idle" || state.preloadActive;
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
    if (!ok) setCommandError(server.error ?? "The command could not be executed.");
  };
  const replaceCommand = (value: string) => {
    setCompleted(false);
    setCommandError(null);
    server.setCommandLine(value);
  };
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("input,textarea,select,[contenteditable=true]"))
        return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const key = event.key.toUpperCase();
      if (key === "ESCAPE") {
        event.preventDefault();
        if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
        else if (persistentError) {
          setPersistentError(null);
          setErrorOpen(false);
          server.dismissError();
        } else replaceCommand("");
        return;
      }
      if (
        !["ENTER", "BACKSPACE", "T", "A", "F", "."].includes(key) &&
        !/^\d$/.test(key)
      )
        return;
      event.preventDefault();
      if (key === "ENTER") {
        void execute();
        return;
      }
      let current = completed ? "" : server.commandLine;
      if (key === "BACKSPACE") {
        replaceCommand(removeCommandToken(current));
        return;
      }
      if (/^\d$/.test(key))
        current = current ? `${current}${key}` : `FIXTURE ${key}`;
      else if (key === ".") current += ".";
      else if (key === "T") current = `${current.trim()} THRU `;
      else if (key === "A") current = `${current.trim()} AT `;
      else if (key === "F") current = `${current.trim()} FULL`;
      replaceCommand(current);
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [
    completed,
    persistentError,
    state.storeArmed,
    server.commandLine,
    server.executeCommandLine,
    server.setCommandLine,
  ]);
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
        className={`command-input ${state.preload === "blind" ? "blind" : ""} ${completed ? "completed" : ""} ${commandError ? "error" : ""}`}
        aria-label="Command line"
        value={server.commandLine}
        placeholder="FIXTURE 1 THRU 8 AT 75"
        onChange={(event) =>
          replaceCommand(
            completed ? event.target.value.slice(-1) : event.target.value,
          )
        }
        onKeyDown={(event) => {
          if (event.key === "Enter") void execute();
        }}
      /><Button className="command-escape" onClick={() => replaceCommand("")}>ESC</Button>
        <Button className={`command-status ${server.status}`} title="Open output and timecode controls" onClick={() => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true })}>
          <span className={state.blackout ? "blackout-status" : ""}>{state.blackout ? <><i>DMX {server.bootstrap?.frame_rate_hz ?? "—"}Hz</i><b>BLACKOUT</b></> : <>DMX {server.bootstrap?.frame_rate_hz ?? "—"}Hz</>}</span>
          <span>{server.bootstrap?.active_timecode ?? "No timecode"}</span>
        </Button>
      </div>
      {completed && (
        <span className="command-complete" aria-label="Command applied">
          ✓
        </span>
      )}
      {errorOpen && persistentError && <div className="persistent-error-popover" role="alertdialog"><header><b><span>▲</span> Desk error</b><Button onClick={() => setErrorOpen(false)}>×</Button></header><pre>{persistentError}</pre><Button onClick={() => { setPersistentError(null); server.dismissError(); setErrorOpen(false); }}>Acknowledge</Button></div>}
      <div className="command-record-preload">
        <Button className={`global-store-button ${state.storeArmed ? "armed" : hasRecordableContent ? "record-ready" : "record-empty"}`} onPointerDown={() => { storeHeld.current = false; storeHold.current = window.setTimeout(() => { storeHeld.current = true; storeSuppressUntil.current = performance.now() + 1000; dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: true }); }, 650); }} onPointerUp={() => { if (storeHold.current) window.clearTimeout(storeHold.current); storeHold.current = null; }} onPointerCancel={() => { if (storeHold.current) window.clearTimeout(storeHold.current); storeHold.current = null; }} onClick={() => { if (!storeHeld.current && performance.now() >= storeSuppressUntil.current) dispatch({ type: "SET_STORE_ARMED", value: !state.storeArmed }); storeHeld.current = false; }}>{state.storeArmed ? "REC ARMED" : "REC"}</Button>
        <Button
          className={`preload-button ${state.preload === "blind" ? "preload-go" : "preload-enter"}`}
          title={state.preloadActive ? "Hold to release the active preload scene" : undefined}
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
          {state.preloadActive && <small>(Hold: release)</small>}
        </Button>
      </div>
    </header>
  );
}
