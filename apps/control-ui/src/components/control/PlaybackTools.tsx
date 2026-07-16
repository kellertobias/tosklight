import { useApp } from "../../state/AppContext";
import { TouchTimeSurface } from "./TouchTimeSurface";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { Button } from "../common";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";

export function PlaybackTools() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const speedBpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15];
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const taps = useRef<Record<string, number[]>>({});
  useEffect(() => {
    const active = server.playbacks?.active_page;
    if (active != null && active - 1 !== state.playbackPage) dispatch({ type: "SET_PLAYBACK_PAGE", page: active - 1 });
  }, [server.playbacks?.active_page, state.playbackPage, dispatch]);
  const tap = (group: "A" | "B" | "C" | "D" | "E") => {
    const now = performance.now();
    const recent = [...(taps.current[group] ?? []), now].filter((time) => now - time < 3000).slice(-6);
    taps.current[group] = recent;
    if (recent.length > 1) {
      const intervals = recent.slice(1).map((time, index) => time - recent[index]);
      const next = Math.round(60000 / (intervals.reduce((sum, value) => sum + value, 0) / intervals.length));
      const values = [...speedBpms] as [number, number, number, number, number];
      values[group.charCodeAt(0) - 65] = next;
      void server.setControlTiming({ speed_groups_bpm: values });
    }
  };
  useEffect(() => {
    const keyboardTap = (event: Event) => tap((event as CustomEvent<"A" | "B" | "C" | "D" | "E">).detail);
    window.addEventListener("light:speed-group-tap", keyboardTap);
    return () => window.removeEventListener("light:speed-group-tap", keyboardTap);
  });
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
      <Button className={state.playbackSetArmed ? "active" : ""} onClick={() => dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: !state.playbackSetArmed })}>SET</Button>
      <div className="playback-page-controls">
        <Button className="playback-page-chevron" aria-label="Previous playback page" disabled={state.playbackPage === 0} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: state.playbackPage - 1 }); void server.setPlaybackPage(state.playbackPage); }}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 15 7-7 7 7"/></svg></Button>
        <Button className="playback-page-current" aria-label={`Select playback page. Page ${state.playbackPage + 1} ${state.playbackPageNames[state.playbackPage]}`} onClick={() => setPagePickerOpen(true)}><span>Page</span><strong>{state.playbackPage + 1}</strong><small>{state.playbackPageNames[state.playbackPage]}</small></Button>
        <Button className="playback-page-chevron" aria-label="Next playback page" disabled={state.playbackPage === state.playbackPageNames.length - 1} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: state.playbackPage + 1 }); void server.setPlaybackPage(state.playbackPage + 2); }}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 9 7 7 7-7"/></svg></Button>
      </div>
      <ProgrammerFadeFader/>
      <div className="cue-fade-master"><TouchTimeSurface
        label="Cue Fade"
        value={(server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000}
        maximum={60}
        display={`${((server.configuration?.sequence_master_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`}
        onChange={(value) => void server.setControlTiming({ sequence_master_fade_millis: Math.round(value * 1_000) })}
      /></div>
      <div className="speed-group-stack">
        {(["A", "B", "C", "D", "E"] as const).map((group, index) => { const bpm = speedBpms[index]; return (
          <Button
            style={{ "--bpm": bpm } as CSSProperties}
            className="active"
            aria-label={`Speed group ${group}, ${bpm} BPM`}
            key={group}
            onClick={() => tap(group)}
          >
            <strong className="speed-group-label">{group}</strong>
            <span className="speed-group-value">{bpm}</span>
            <small className="speed-group-unit">BPM</small>
          </Button>
        ); })}
      </div>
      {pagePickerOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setPagePickerOpen(false)}><div className="nested-modal playback-page-modal" role="dialog" aria-modal="true" aria-label="Playback pages"><Button className="modal-close" onClick={() => setPagePickerOpen(false)}>×</Button><h3>Playback pages</h3><div>{(server.playbacks?.pages ?? []).map((item) => <Button className={item.number === (server.playbacks?.active_page ?? 1) ? "active" : ""} key={item.number} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: item.number - 1 }); void server.setPlaybackPage(item.number); setPagePickerOpen(false); }}><strong>{item.number}</strong><span>{item.name}</span></Button>)}</div></div></div>, document.body)}
    </div>
  );
}
