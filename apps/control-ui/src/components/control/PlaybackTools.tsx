import { useApp } from "../../state/AppContext";
import { TouchTimeSurface } from "./TouchTimeSurface";
import { useEffect, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { Button } from "../common";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";
import { SoundToLightModal } from "./SoundToLightModal";
import { inactiveCaptureStatus, monotonicEpochMillis } from "./soundToLightAnalyzer";
import { useSoundToLight, type SoundToLightController } from "./useSoundToLight";
import type { SpeedGroupId } from "../../api/types";

export function PlaybackTools() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const speedBpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15];
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const [soundGroup, setSoundGroup] = useState<SpeedGroupId | null>(null);
  const sound = useSoundToLight();
  useEffect(() => {
    const active = server.playbacks?.active_page;
    if (active != null && active - 1 !== state.playbackPage) dispatch({ type: "SET_PLAYBACK_PAGE", page: active - 1 });
  }, [server.playbacks?.active_page, state.playbackPage, dispatch]);
  useEffect(() => {
    const keyboardTap = (event: Event) => void sound.action((event as CustomEvent<SpeedGroupId>).detail, {
      action: "learn",
      captured_at_millis: monotonicEpochMillis(),
    });
    window.addEventListener("light:speed-group-tap", keyboardTap);
    return () => window.removeEventListener("light:speed-group-tap", keyboardTap);
  }, [sound.action]);
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
        {(["A", "B", "C", "D", "E"] as const).map((group, index) => { const speedState = sound.states[group]; const bpm = speedState?.snapshot.effective_bpm ?? speedBpms[index]; const displayBpm = Number.isInteger(bpm) ? String(bpm) : bpm.toFixed(1); return (
          <Button
            style={{ "--bpm": bpm } as CSSProperties}
            className={`active ${speedState?.configuration.enabled ? "sound-enabled" : ""}`}
            aria-label={`Speed group ${group}, ${displayBpm} BPM`}
            title={`Open Speed Group ${group} Sound-to-Light configuration`}
            key={group}
            onClick={() => setSoundGroup(group)}
          >
            <strong className="speed-group-label">{group}</strong>
            <span className="speed-group-value">{displayBpm}</span>
            <small className="speed-group-unit">BPM</small>
          </Button>
        ); })}
      </div>
      {pagePickerOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setPagePickerOpen(false)}><div className="nested-modal playback-page-modal" role="dialog" aria-modal="true" aria-label="Playback pages"><Button className="modal-close" onClick={() => setPagePickerOpen(false)}>×</Button><h3>Playback pages</h3><div>{(server.playbacks?.pages ?? []).map((item) => <Button className={item.number === (server.playbacks?.active_page ?? 1) ? "active" : ""} key={item.number} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: item.number - 1 }); void server.setPlaybackPage(item.number); setPagePickerOpen(false); }}><strong>{item.number}</strong><span>{item.name}</span></Button>)}</div></div></div>, document.body)}
      {soundGroup && sound.states[soundGroup] && <SoundToLightModal
        group={soundGroup}
        state={sound.states[soundGroup]!}
        capture={sound.captures[soundGroup] ?? inactiveCaptureStatus}
        permission={sound.permission}
        devices={sound.devices}
        deviceId={sound.deviceIds[soundGroup] ?? ""}
        controllerError={sound.error}
        onDeviceChange={(deviceId) => sound.setDevice(soundGroup, deviceId)}
        onRefreshInputs={sound.refreshInputs}
        onPreview={sound.setPreview}
        onSave={(configuration) => sound.save(soundGroup, configuration)}
        onAction={(input) => sound.action(soundGroup, input)}
        onClose={() => setSoundGroup(null)}
      />}
      {soundGroup && !sound.states[soundGroup] && <SoundToLightLoading group={soundGroup} controller={sound} onClose={() => setSoundGroup(null)}/>} 
    </div>
  );
}

function SoundToLightLoading({ group, controller, onClose }: { group: SpeedGroupId; controller: SoundToLightController; onClose: () => void }) {
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}><section className="nested-modal" role="dialog" aria-modal="true" aria-label={`Speed Group ${group} Sound to Light`}><Button className="modal-close" aria-label="Close Sound-to-Light configuration" onClick={onClose}>×</Button><h3>Speed Group {group} · Sound to Light</h3><p>{controller.loading ? "Loading Speed Group configuration…" : controller.error ?? "Speed Group configuration is not available."}</p></section></div>, document.body);
}
