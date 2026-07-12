import { useApp } from "../../state/AppContext";
import { TouchTimeSurface } from "./TouchTimeSurface";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";

export function PlaybackTools() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const speedBpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15];
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const taps = useRef<Record<string, number[]>>({});
  const tap = (group: "A" | "B" | "C" | "D" | "E") => {
    const now = performance.now();
    const recent = [...(taps.current[group] ?? []), now].filter((time) => now - time < 3000).slice(-6);
    taps.current[group] = recent;
    dispatch({ type: "SET_SPEED_GROUP", value: group });
    if (recent.length > 1) {
      const intervals = recent.slice(1).map((time, index) => time - recent[index]);
      const next = Math.round(60000 / (intervals.reduce((sum, value) => sum + value, 0) / intervals.length));
      const values = [...speedBpms] as [number, number, number, number, number];
      values[group.charCodeAt(0) - 65] = next;
      void server.setControlTiming({ speed_groups_bpm: values });
    }
  };
  useEffect(() => {
    if (!pagePickerOpen) return;
    const close = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault(); event.stopImmediatePropagation(); setPagePickerOpen(false);
    };
    window.addEventListener("keydown", close, true);
    return () => window.removeEventListener("keydown", close, true);
  }, [pagePickerOpen]);
  return (
    <div className="playback-tools">
      <div className="speed-group-stack">
        {(["A", "B", "C", "D", "E"] as const).map((group, index) => { const bpm = speedBpms[index]; return (
          <button
            style={{ "--bpm": bpm } as CSSProperties}
            className={`active ${state.speedGroup === group ? "selected" : ""}`}
            key={group}
            onClick={() => tap(group)}
          >
            <strong>{group}</strong><small>{bpm}<i>BPM</i></small>
          </button>
        ); })}
      </div>
      <div className="cue-fade-master"><TouchTimeSurface
        label="Cue Fade"
        value={(server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000}
        maximum={60}
        display={`${((server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`}
        onChange={(value) => void server.setControlTiming({ sequence_master_fade_millis: Math.round(value * 1_000) })}
      /></div>
      <div className="playback-page-controls">
        <button disabled={state.playbackPage === 0} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: state.playbackPage - 1 }); void server.setPlaybackPage(state.playbackPage); }}><span>▲</span> PAGE UP</button>
        <button className="playback-page-current" onClick={() => setPagePickerOpen(true)}><strong>{state.playbackPage + 1}</strong><span>{state.playbackPageNames[state.playbackPage]}</span></button>
        <button disabled={state.playbackPage === state.playbackPageNames.length - 1} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: state.playbackPage + 1 }); void server.setPlaybackPage(state.playbackPage + 2); }}>PAGE DOWN <span>▼</span></button>
      </div>
      {pagePickerOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setPagePickerOpen(false)}><div className="nested-modal playback-page-modal" role="dialog" aria-modal="true" aria-label="Playback pages"><button className="modal-close" onClick={() => setPagePickerOpen(false)}>×</button><h3>Playback pages</h3><div>{(server.playbacks?.pages ?? []).map((item) => <button className={item.number === (server.playbacks?.active_page ?? 1) ? "active" : ""} key={item.number} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: item.number - 1 }); void server.setPlaybackPage(item.number); setPagePickerOpen(false); }}><strong>{item.number}</strong><span>{item.name}</span></button>)}</div></div></div>, document.body)}
    </div>
  );
}
