import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader, type VerticalTouchFaderAction } from "./VerticalTouchFader";
import { useApp } from "../../state/AppContext";
import { playbackSlotNumbers } from "./playbackProjection";
import { Button } from "../common";
import { useEffect, useState, type CSSProperties, type MouseEvent as ReactMouseEvent } from "react";
import type { Cue } from "../../api/types";
import type { PlaybackDefinition } from "../../api/types";
import { PlaybackConfigurationModal } from "./PlaybackConfigurationModal";
import { isSetContextClick } from "../../disableContextMenu";

function HardwareCueRows({ cues, cueIndex, activatedAt, compact, effectiveNextCueNumber, effectiveNextIsLoaded }: { cues: Cue[]; cueIndex: number; activatedAt?: string; compact: boolean; effectiveNextCueNumber?: number | null; effectiveNextIsLoaded?: boolean }) {
  const [now, setNow] = useState(() => Date.now());
  const current = cues[cueIndex];
  useEffect(() => { setNow(Date.now()); if (!current?.fade_millis || !activatedAt) return; const timer = window.setInterval(() => setNow(Date.now()), 50); return () => window.clearInterval(timer); }, [current?.fade_millis, cueIndex, activatedAt]);
  const elapsed = activatedAt ? now - Date.parse(activatedAt) : Number.POSITIVE_INFINITY;
  const progress = current?.fade_millis && elapsed < current.fade_millis ? elapsed / current.fade_millis : 0;
  const effectiveNextIndex = effectiveNextCueNumber == null ? -1 : cues.findIndex((cue) => cue.number === effectiveNextCueNumber);
  const effectiveNext = effectiveNextIndex < 0 ? undefined : cues[effectiveNextIndex];
  const rows = compact
    ? effectiveNextIsLoaded ? [[effectiveNext, effectiveNextIndex, "next"] as const] : [[current, cueIndex, "current"] as const]
    : [[cues[cueIndex - 1], cueIndex - 1, "previous"] as const, [current, cueIndex, "current"] as const, [effectiveNext, effectiveNextIndex, "next"] as const];
  return <div className={`hardware-cue-list ${compact ? "single" : "triple"}`}>{rows.map(([cue, index, kind]) => <div className={`hardware-cue-row ${kind} ${kind === "next" && effectiveNextIsLoaded ? "loaded-next" : ""}`} style={kind === "current" ? { "--cue-fade-progress": progress } as CSSProperties : undefined} key={`${kind}-${index}`}><i/><span>{cue?.number ?? "—"}</span><b>{cue?.name || (cue ? `Cue ${cue.number}` : "—")}</b><small>{kind === "next" && effectiveNextIsLoaded ? "LOADED NEXT" : cue?.fade_millis ? `${(cue.fade_millis / 1000).toFixed(1)}s` : ""}</small></div>)}</div>;
}

export interface PlaybackFaderBankProps { pageNumber?: number; firstSlot?: number; count?: number; rows?: number; buttons?: number }
export function PlaybackFaderBank({ pageNumber, firstSlot = 1, count, rows, buttons }: PlaybackFaderBankProps = {}) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const pageSize = count ?? state.playbackColumns * state.playbackRows;
  const rowCount = rows ?? state.playbackRows;
  const columns = Math.ceil(pageSize / rowCount);
  const activePageNumber = pageNumber ?? server.playbacks?.active_page ?? state.playbackPage + 1;
  const page = server.playbacks?.pages.find((candidate) => candidate.number === activePageNumber);
  const [configuration, setConfiguration] = useState<{ playback: PlaybackDefinition; page: number; slot: number } | null>(null);
  const assignmentPending = state.cueListSetTarget != null;
  const selectionPending = /^SELECT\s*$/i.test(server.commandLine);
  const selectPlayback = async (event: ReactMouseEvent, playback: PlaybackDefinition | null) => {
    if (!selectionPending || !playback) return;
    event.preventDefault();
    event.stopPropagation();
    await server.poolPlaybackAction(playback.number, "select");
    server.resetCommandLine();
    await server.refresh();
  };
  const assignPlayback = async (slot: number) => {
    if (state.cueListSetTarget == null) return;
    const ok = await server.executeCommandLine(`SET ${state.cueListSetTarget} AT ${activePageNumber}.${slot}`);
    if (!ok) return;
    await server.refresh();
    dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
  };
  const slots = playbackSlotNumbers(page, firstSlot, pageSize).map((number) => {
    const playback = server.playbacks?.pool.find((candidate) => candidate.number === number) ?? null;
    const cueListId = playback?.target.type === "cue_list" ? playback.target.cue_list_id : null;
    const cue = cueListId ? server.playbacks?.cue_lists.find((candidate) => candidate.id === cueListId) ?? null : null;
    const groupId = playback?.target.type === "group" ? playback.target.group_id : null;
    const group = groupId ? server.groups.find((candidate) => candidate.id === groupId) ?? null : null;
    return { playback, cue, group };
  });
  const openConfiguration = (playback: PlaybackDefinition, slot: number) => {
    setConfiguration({ playback, page: activePageNumber, slot });
    dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
    dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
    dispatch({ type: "SET_SHIFT_ARMED", value: false });
  };
  const emptyConfiguration = (slot: number): PlaybackDefinition => ({ number: 1000 - slot + 1, name: "Empty Playback", target: { type: "cue_list", cue_list_id: server.playbacks?.cue_lists[0]?.id ?? "" }, buttons: ["go_minus", "go", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false });
  const setClickArmed = () => state.playbackSetArmed || (state.cueListSetArmed && state.cueListSetTarget == null);
  return <><div className="playback-fader-bank" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${rowCount}, minmax(0, 1fr))` }}>
    {slots.map(({ playback, cue, group }, index) => {
      const slot = firstSlot + index;
      const active = playback ? server.playbacks?.active.find((item) => item.playback_number === playback.number) : undefined;
      const selected = playback?.number === server.playbacks?.selected_playback;
      const value = Math.round((group ? group.body.master ?? 1 : active?.master ?? 0) * 100);
      const actions = (playback?.buttons ?? ["none", "none", "none"]).slice(0, hardware ? 3 : buttons ?? server.playbacks?.desk.buttons ?? 3);
      const faderActions: VerticalTouchFaderAction[] = actions.map((action, button) => ({ id: `${button}-${action}`, label: action === "go_minus" ? "GO −" : action.toUpperCase(), disabled: assignmentPending || !playback || action === "none",
        onClick: (event) => {
          if (!playback) return;
          if (setClickArmed() || (button === 0 && (event.shiftKey || state.shiftArmed))) {
            event.stopPropagation();
            openConfiguration(playback, slot);
            return;
          }
          if (action !== "flash" && action !== "none") void server.poolPlaybackAction(playback.number, action.replaceAll("_", "-") as Parameters<typeof server.poolPlaybackAction>[1]);
        },
        onPointerDown: (event) => { if (!playback || action !== "flash") return; event.currentTarget.setPointerCapture(event.pointerId); void server.poolPlaybackAction(playback.number, "flash", { pressed: true }); },
        onPointerUp: () => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false }),
        onPointerCancel: () => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false }),
        onLostPointerCapture: () => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false }),
      }));
      const actionButtons = faderActions.map(({ id, label, ...props }) => <Button {...props} key={id}>{label}</Button>);
      const assignmentTarget = assignmentPending && <Button className="playback-assignment-target" aria-label={`Assign Cuelist ${state.cueListSetTarget} to page ${activePageNumber} playback ${firstSlot + index}`} onClick={() => void assignPlayback(firstSlot + index)}><b>Assign Cuelist {state.cueListSetTarget}</b><small>to playback {activePageNumber}.{firstSlot + index}</small></Button>;
      const configurationTarget = !assignmentPending && setClickArmed() && <Button className="playback-assignment-target playback-configuration-target" aria-label={`Configure page ${activePageNumber} playback ${slot}`} onClick={(event) => { event.stopPropagation(); openConfiguration(playback ?? emptyConfiguration(slot), slot); }}><b>Configure Playback</b><small>{activePageNumber}.{slot} · {playback?.name ?? "Empty"}</small></Button>;
      if (hardware) {
        const cueIndex = active?.enabled === false ? -1 : active?.cue_index ?? -1;
        return <article data-set-click-target data-selected-playback={selected || undefined} data-selection-pending={selectionPending || undefined} className={`hardware-playback-card ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${assignmentPending ? "assignment-pending" : ""}`} key={playback?.number ?? `empty-${index}`} onClickCapture={(event) => void selectPlayback(event, playback)} onClick={(event: ReactMouseEvent) => { if (playback && (isSetContextClick(event.nativeEvent) || setClickArmed())) openConfiguration(playback, slot); }}>
          {assignmentTarget}
          {configurationTarget}
          <header><b>{playback?.name ?? "—"}</b><strong>{page?.number ?? pageNumber ?? state.playbackPage + 1}.{firstSlot + index}</strong></header>
          {cue ? <HardwareCueRows cues={cue.cues} cueIndex={cueIndex} activatedAt={active?.activated_at} compact={rowCount === 2} effectiveNextCueNumber={active?.effective_next_cue_number} effectiveNextIsLoaded={active?.effective_next_is_loaded} /> : group ? <div className="hardware-cue-list single"><div className="hardware-cue-row current"><i/><span>GRP</span><b>{group.body.name ?? `Group ${group.id}`}</b><small>{value}% master</small></div></div> : <div className="hardware-cue-list single" />}
          <div className="hardware-playback-controls"><footer>{actionButtons}</footer><div className="hardware-fader" style={{ "--hardware-fader-level": `${value}%` } as CSSProperties}><i/><b>{value}%</b></div></div>
        </article>;
      }
      return <article data-set-click-target data-selected-playback={selected || undefined} data-selection-pending={selectionPending || undefined} className={`${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${assignmentPending ? "assignment-pending" : ""}`} key={playback?.number ?? `empty-${index}`} onClickCapture={(event) => void selectPlayback(event, playback)} onClick={(event: ReactMouseEvent) => { if (playback && (isSetContextClick(event.nativeEvent) || setClickArmed())) openConfiguration(playback, slot); }}>
        {assignmentTarget}
        {configurationTarget}
        <b>{firstSlot + index} · {playback?.name ?? "—"}</b>
        <VerticalTouchFader disabled={assignmentPending || !playback || playback.fader === "speed"} label={group ? "group master" : playback?.fader ?? "Empty"} value={value}
          display={active?.loaded_cue_number != null ? `Load ${active.loaded_cue_number} · ${value}%` : active?.enabled !== false && active && cue ? `Cue ${active.current_cue_number ?? active.cue_index + 1} · ${value}%` : group ? `${value}% master` : playback ? `${value}%` : "Empty"}
          actions={faderActions}
          onChange={(next) => playback && void server.poolPlaybackAction(playback.number, "master", { value: next / 100 })}/>
      </article>;
    })}
  </div>{configuration && <PlaybackConfigurationModal playback={configuration.playback} page={configuration.page} slot={configuration.slot} onClose={() => setConfiguration(null)} onUnassign={() => server.unassignPagePlayback(configuration.page, configuration.slot)}/>}</>;
}
