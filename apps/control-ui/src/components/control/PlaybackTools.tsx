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
import { editTargetedCommandWithSoftwareKey, type SoftwareKey } from "./softwareKeypad";
import type { SpeedGroupId } from "../../api/types";
import { canAdvancePlaybackPage, PlaybackPageMenu, PlaybackPageRenameDialog } from "./PlaybackPageDialogs";
import { usePlaybackDeskView } from "../../features/playbackRuntime/PlaybackRuntimeView";
import { useCommandLineSurface } from "./commandLine/useCommandLineSurface";

export function PlaybackTools() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const command = useCommandLineSurface({ observeCommand: false });
  const playbackDesk = usePlaybackDeskView();
  const speedBpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15];
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const [pageRenameOpen, setPageRenameOpen] = useState(false);
  const [soundGroup, setSoundGroup] = useState<SpeedGroupId | null>(null);
  const sound = useSoundToLight();
  useEffect(() => {
    const active = playbackDesk?.active_page;
    if (active != null && active - 1 !== state.playbackPage) dispatch({ type: "SET_PLAYBACK_PAGE", page: active - 1 });
  }, [playbackDesk?.active_page, state.playbackPage, dispatch]);
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
      if (document.querySelector(".ui-input-modal-layer")) return;
      event.preventDefault(); event.stopImmediatePropagation(); setPagePickerOpen(false);
    };
    window.addEventListener("keydown", close, true);
    return () => window.removeEventListener("keydown", close, true);
  }, [pagePickerOpen]);
  const pressCommandKey = (key: SoftwareKey) => {
    const currentCommand = command.read();
    if (key === "SHIFT") {
      dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed });
      return;
    }
    if (key === "SET") {
      dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: !state.playbackSetArmed });
      return;
    }
    if (state.shiftArmed) {
      dispatch({ type: "SET_SHIFT_ARMED", value: false });
      if (key === "DEL") {
        dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
        return;
      }
    }
    const edited = editTargetedCommandWithSoftwareKey(currentCommand.text, key, currentCommand.target, currentCommand.pristine);
    void command.replace(edited.command, edited.pristine);
    if (edited.execute) void command.execute(edited.command);
  };
  const pages = server.playbacks?.pages ?? [];
  const activePageNumber = playbackDesk?.active_page ?? state.playbackPage + 1;
  const activePage = pages.find((page) => page.number === activePageNumber) ?? null;
  const selectCurrentPage = () => {
    if (state.playbackSetArmed && activePage) {
      dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
      setPageRenameOpen(true);
      return;
    }
    setPagePickerOpen(true);
  };
  const nextPage = async () => {
    const target = activePageNumber + 1;
    if (!pages.some((page) => page.number === target)) {
      const saved = await server.savePlaybackPage({ number: target, name: `Page ${target}`, slots: {} });
      if (!saved) return;
    }
    dispatch({ type: "SET_PLAYBACK_PAGE", page: target - 1 });
    await server.setPlaybackPage(target);
  };
  return (
    <div className="playback-tools">
      <div className="playback-command-keys">
        {(["SET", "CPY", "MOV", "DEL", "SHIFT"] as const).map((key) => <Button
          className={(key === "SET" && state.playbackSetArmed) || (key === "SHIFT" && state.shiftArmed) ? "active" : ""}
          data-keypad-key={key}
          key={key}
          onClick={() => pressCommandKey(key)}
        >{key}</Button>)}
      </div>
      <div className="playback-page-controls">
        <Button className="playback-page-chevron" aria-label="Previous playback page" disabled={state.playbackPage === 0} onClick={() => { dispatch({ type: "SET_PLAYBACK_PAGE", page: state.playbackPage - 1 }); void server.setPlaybackPage(state.playbackPage); }}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 15 7-7 7 7"/></svg></Button>
        <Button className="playback-page-current" aria-label={`Select playback page. Page ${activePageNumber} ${activePage?.name ?? `Page ${activePageNumber}`}`} onClick={selectCurrentPage}><span>Page</span><strong>{activePageNumber}</strong><small>{activePage?.name ?? `Page ${activePageNumber}`}</small></Button>
        <Button className="playback-page-chevron" aria-label="Next playback page" disabled={!canAdvancePlaybackPage(pages, activePageNumber)} onClick={() => void nextPage()}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 9 7 7 7-7"/></svg></Button>
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
      <PlaybackPageMenu open={pagePickerOpen} onClose={() => setPagePickerOpen(false)}/>
      <PlaybackPageRenameDialog page={pageRenameOpen ? activePage : null} onClose={() => setPageRenameOpen(false)}/>
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
