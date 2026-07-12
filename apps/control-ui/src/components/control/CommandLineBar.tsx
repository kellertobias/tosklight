import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

export function CommandLineBar() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const playback = state.controlMode === "playbacks";
  const preloadLabel = state.preload === "blind" ? "PRELOAD GO" : "PRELOAD";
  const advancePreload = async () => { await server.preloadAction(state.preload === "blind" ? "go" : "enter"); dispatch({ type: "ADVANCE_PRELOAD" }); };
  const releasePreload = async () => { await server.preloadAction("release"); dispatch({ type: "RELEASE_PRELOAD" }); };
  return <header className={`command-line-bar ${playback ? "playback-mode" : ""}`}>
    <div className="command-line-left">
      <button className={`mode-toggle ${playback ? "playbacks-active" : ""}`} onClick={() => dispatch({ type: "TOGGLE_CONTROL_MODE" })}><span className="mode-icon">{playback ? "▶" : "⌨"}</span><span><b>PROGRAMMER</b><small>PLAYBACKS</small></span></button>
      <input className={`command-input ${state.preload === "blind" ? "blind" : ""}`} aria-label="Command line" value={server.commandLine} placeholder="FIXTURE 1 THRU 8 AT 75" onChange={(event) => server.setCommandLine(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void server.executeCommandLine(); }} />
      <label className="header-fade">Fade<input value={`${state.programmerFade.toFixed(1)} s`} readOnly /></label>
      <button className={`command-status ${server.status}`} title="Open output and programmer controls" onClick={() => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true })}><span>● DMX {server.bootstrap?.frame_rate_hz ?? "—"} Hz · {server.status === "connected" ? "OK" : server.status.toUpperCase()}</span><span>{server.error ? `⚠ ${server.error}` : `◆ TC ${server.bootstrap?.active_timecode_source ?? "No source"}`}</span></button>
      {state.preloadActive && <button className="preload-scene" onClick={() => void releasePreload()}><b>Preload Scene</b><small>{state.preload === "blind" ? "Active · pending edit" : "Release"}</small></button>}
      <button className="preload-button" onClick={() => void advancePreload()}>{preloadLabel}</button>
    </div>
    <div className="command-line-right" aria-hidden="true" />
  </header>;
}
