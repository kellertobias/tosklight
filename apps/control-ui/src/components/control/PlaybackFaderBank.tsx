import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader, type VerticalTouchFaderAction } from "./VerticalTouchFader";
import { useApp } from "../../state/AppContext";
import { playbackSlotNumbers } from "./playbackProjection";
import { Button, Input } from "../common";
import { useEffect, useState, type CSSProperties, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from "react";
import type { Cue } from "../../api/types";
import type { PlaybackDefinition, PlaybackSurfaceLayout, PlaybackSurfaceRow } from "../../api/types";
import { normalizePlaybackTopology, PlaybackConfigurationModal } from "./PlaybackConfigurationModal";
import { isSetContextClick } from "../../disableContextMenu";
import { cueUpdateTarget, requestUpdateTarget } from "./updateWorkflow";

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

export function playbackRowUnits(row: PlaybackSurfaceRow, hardware: boolean) {
  if (hardware) return row.has_fader ? 2 : 1;
  return row.has_fader ? 4 : row.button_count > 1 ? 2 : 1;
}

export interface PlaybackFaderBankProps { pageNumber?: number; firstSlot?: number; count?: number; rows?: number; buttons?: number; playbackLayout?: PlaybackSurfaceLayout | null }
export function PlaybackFaderBank({ pageNumber, firstSlot = 1, count, rows, buttons, playbackLayout }: PlaybackFaderBankProps = {}) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const pageSize = count ?? state.playbackColumns * state.playbackRows;
  const rowCount = playbackLayout?.rows.length ?? rows ?? state.playbackRows;
  const columns = playbackLayout?.playbacks_per_row ?? Math.ceil(pageSize / rowCount);
  const activePageNumber = pageNumber ?? server.playbacks?.active_page ?? state.playbackPage + 1;
  const page = server.playbacks?.pages.find((candidate) => candidate.number === activePageNumber);
  const [configuration, setConfiguration] = useState<{ playback: PlaybackDefinition; page: number; slot: number; empty: boolean } | null>(null);
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
  const recordPlayback = async (event: ReactMouseEvent, playback: PlaybackDefinition | null, slot: number) => {
    if (!state.storeArmed) return;
    event.preventDefault();
    event.stopPropagation();
    const cueListId = playback?.target.type === "cue_list" ? playback.target.cue_list_id : undefined;
    await server.storePlayback(slot - 1, cueListId);
    dispatch({ type: "SET_STORE_ARMED", value: false });
  };
  const assignPlayback = async (slot: number) => {
    if (state.cueListSetTarget == null) return;
    const ok = await server.executeCommandLine(`SET ${state.cueListSetTarget} AT ${activePageNumber}.${slot}`);
    if (!ok) return;
    await server.refresh();
    dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
  };
  const slotCells = playbackLayout
    ? playbackLayout.rows.flatMap((row, rowIndex) => Array.from({ length: columns }, (_, columnIndex) => ({ slot: row.first_playback_slot + columnIndex, row, rowIndex })))
    : playbackSlotNumbers(page, firstSlot, pageSize).map((_, index) => ({ slot: firstSlot + index, row: null, rowIndex: Math.floor(index / columns) }));
  const slots = slotCells.map(({ slot, row, rowIndex }) => {
    const number = page?.slots[String(slot)];
    const playback = server.playbacks?.pool.find((candidate) => candidate.number === number) ?? null;
    const cueListId = playback?.target.type === "cue_list" ? playback.target.cue_list_id : null;
    const cue = cueListId ? server.playbacks?.cue_lists.find((candidate) => candidate.id === cueListId) ?? null : null;
    const groupId = playback?.target.type === "group" ? playback.target.group_id : null;
    const group = groupId ? server.groups.find((candidate) => candidate.id === groupId) ?? null : null;
    return { playback, cue, group, slot, row, rowIndex };
  });
  const openConfiguration = (playback: PlaybackDefinition | null, slot: number) => {
    const fallbackButtons = Math.max(0, Math.min(3, buttons ?? server.playbacks?.desk.buttons ?? 3));
    setConfiguration({ playback: normalizePlaybackTopology(playback ?? emptyConfiguration(activePageNumber, slot, fallbackButtons, true, server.playbacks?.cue_lists[0]?.id ?? ""), fallbackButtons, true), page: activePageNumber, slot, empty: !playback });
    dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
    dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
    dispatch({ type: "SET_SHIFT_ARMED", value: false });
  };
  const setClickArmed = () => state.playbackSetArmed || (state.cueListSetArmed && state.cueListSetTarget == null);
  const rowTracks = playbackLayout
    ? playbackLayout.rows.map((row) => `minmax(0, ${playbackRowUnits(row, hardware)}fr)`).join(" ")
    : `repeat(${rowCount}, minmax(0, 1fr))`;
  return <><div className={`playback-fader-bank ${hardware ? "hardware-layout" : "touch-layout"}`} style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gridTemplateRows: rowTracks }}>
    {slots.map(({ playback, cue, group, slot, row, rowIndex }, index) => {
      const active = playback ? server.playbacks?.active.find((item) => item.playback_number === playback.number) : undefined;
      const selected = playback?.number === server.playbacks?.selected_playback;
      const configuredButtons = row?.button_count ?? (hardware ? 3 : buttons ?? server.playbacks?.desk.buttons ?? 3);
      const buttonCount = playback ? Math.min(configuredButtons, playback.button_count ?? configuredButtons) : configuredButtons;
      const hasFader = (row?.has_fader ?? true) && (playback?.has_fader ?? true);
      const value = playbackFaderValue(playback, active, group?.body.master, server.configuration, server.playbacks?.authoritative_controls, 1);
      const currentCue = cue && active && active.cue_index >= 0 ? cue.cues[active.cue_index] : null;
      const requestPlaybackUpdate = () => {
        if (!playback || playback.target.type !== "cue_list") return;
        requestUpdateTarget(cueUpdateTarget(playback.target.cue_list_id, playback.number, currentCue?.id ? { id: currentCue.id, number: currentCue.number } : null));
      };
      const actions = (playback?.buttons ?? ["none", "none", "none"]).slice(0, buttonCount);
      const faderActions: VerticalTouchFaderAction[] = actions.map((action, button) => {
        const releaseHeldAction = (event: ReactPointerEvent<HTMLButtonElement>) => {
          if (!playback || !isHeldAction(action) || event.currentTarget.dataset.playbackHeld !== "true") return;
          delete event.currentTarget.dataset.playbackHeld;
          void server.poolPlaybackAction(playback.number, "button", { button: button + 1, pressed: false, surface: "physical" });
        };
        return { id: `${button}-${action}`, label: action === "pause" && active?.paused ? "RESUME" : playbackButtonLabel(action), disabled: assignmentPending || !playback || action === "none", className: buttonFeedbackClass(action, active, selected, state.blackout), style: playback ? { "--playback-color": playback.color ?? "#20c997" } as CSSProperties : undefined,
          "data-playback-button-index": button + 1,
          onClick: (event) => {
            if (!playback) return;
            if (state.updateArmed) { event.preventDefault(); event.stopPropagation(); requestPlaybackUpdate(); return; }
            if (setClickArmed() || (button === 0 && (event.shiftKey || state.shiftArmed))) {
              event.stopPropagation();
              openConfiguration(playback, slot);
              return;
            }
            if (!isHeldAction(action) && action !== "none") void server.poolPlaybackAction(playback.number, "button", { button: button + 1, pressed: true, surface: "physical" });
          },
          onPointerDown: (event) => {
            if (state.updateArmed) { event.preventDefault(); event.stopPropagation(); return; }
            if (!playback || !isHeldAction(action)) return;
            event.currentTarget.dataset.playbackHeld = "true";
            event.currentTarget.setPointerCapture?.(event.pointerId);
            void server.poolPlaybackAction(playback.number, "button", { button: button + 1, pressed: true, surface: "physical" });
          },
          onPointerUp: releaseHeldAction,
          onPointerCancel: releaseHeldAction,
          onLostPointerCapture: releaseHeldAction,
        };
      });
      const actionButtons = faderActions.map(({ id, label, ...props }) => <Button {...props} key={id}>{label}</Button>);
      const touchActions = faderActions.filter((_, button) => actions[button] !== "none");
      const touchActionButtons = touchActions.map(({ id, label, ...props }) => <Button {...props} key={id}>{label}</Button>);
      const assignmentTarget = assignmentPending && <Button className="playback-assignment-target" aria-label={`Assign Cuelist ${state.cueListSetTarget} to page ${activePageNumber} playback ${slot}`} onClick={() => void assignPlayback(slot)}><b>Assign Cuelist {state.cueListSetTarget}</b><small>to playback {activePageNumber}.{slot}</small></Button>;
      const configurationTarget = !assignmentPending && setClickArmed() && <div className="playback-assignment-target playback-configuration-target" aria-hidden="true"><b>Configure Playback</b><small>{activePageNumber}.{slot} · {playback?.name ?? "Empty"}</small></div>;
      const interceptPointer = (event: ReactPointerEvent<HTMLElement>) => {
        if (state.updateArmed) { event.preventDefault(); event.stopPropagation(); return; }
        if (state.storeArmed) { event.preventDefault(); event.stopPropagation(); return; }
        const firstButton = (event.target as Element).closest('[data-playback-button-index="1"]');
        if (!setClickArmed() && !(firstButton && state.shiftArmed)) return;
        event.preventDefault(); event.stopPropagation(); openConfiguration(playback, slot);
      };
      const interceptClick = (event: ReactMouseEvent<HTMLElement>) => {
        if (state.updateArmed) { event.preventDefault(); event.stopPropagation(); requestPlaybackUpdate(); return; }
        if (state.storeArmed) { void recordPlayback(event, playback, slot); return; }
        const firstButton = (event.target as Element).closest('[data-playback-button-index="1"]');
        if (setClickArmed() || (firstButton && (event.shiftKey || state.shiftArmed))) { event.preventDefault(); event.stopPropagation(); openConfiguration(playback, slot); return; }
        void selectPlayback(event, playback);
      };
      const cardStyle = playback ? { "--playback-color": playback.color ?? "#20c997" } as CSSProperties : undefined;
      const representation = <Button className="playback-software-representation" aria-label={`Playback representation page ${activePageNumber} playback ${slot}`}><b>{slot} · {playback?.name ?? "Empty"}</b></Button>;
      if (hardware) {
        const cueIndex = active?.enabled === false ? -1 : active?.cue_index ?? -1;
        return <article data-set-click-target data-page={activePageNumber} data-playback-slot={slot} data-playback-row={rowIndex} data-row-units={row ? playbackRowUnits(row, hardware) : 1} data-selected-playback={selected || undefined} data-selection-pending={selectionPending || undefined} className={`hardware-playback-card playback-colored ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.fader_pickup_required ? "pickup-required" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${assignmentPending ? "assignment-pending" : ""} ${state.updateArmed ? "update-target" : ""}`} style={cardStyle} key={`${slot}-${playback?.number ?? "empty"}`} onPointerDownCapture={interceptPointer} onClickCapture={interceptClick} onClick={(event: ReactMouseEvent) => { if (isSetContextClick(event.nativeEvent)) openConfiguration(playback, slot); }}>
          {assignmentTarget}
          {configurationTarget}
          <header>{representation}<strong>{page?.number ?? pageNumber ?? state.playbackPage + 1}.{slot}</strong></header>
          {cue ? <HardwareCueRows cues={cue.cues} cueIndex={cueIndex} activatedAt={active?.activated_at} compact={rowCount === 2} effectiveNextCueNumber={active?.effective_next_cue_number} effectiveNextIsLoaded={active?.effective_next_is_loaded} /> : group ? <div className="hardware-cue-list single"><div className="hardware-cue-row current"><i/><span>GRP</span><b>{group.body.name ?? `Group ${group.id}`}</b><small>{value}% master</small></div></div> : <div className="hardware-cue-list single" />}
          <div className="hardware-playback-controls"><footer>{actionButtons}</footer>{hasFader && <label className="hardware-fader" style={{ "--hardware-fader-level": `${value}%` } as CSSProperties}><i/><b>{playbackFaderDisplay(playback, active, value, server.configuration, server.playbacks?.authoritative_controls, state.blackout)}</b><Input aria-label={`Page ${activePageNumber} playback ${slot} fader`} type="range" min="0" max="100" step="0.1" value={value} onInput={(event) => playback && void server.poolPlaybackAction(playback.number, "master", { value: Number(event.currentTarget.value) / 100, surface: "physical" })}/></label>}</div>
        </article>;
      }
      return <article data-set-click-target data-page={activePageNumber} data-playback-slot={slot} data-playback-row={rowIndex} data-row-units={row ? playbackRowUnits(row, hardware) : 1} data-selected-playback={selected || undefined} data-selection-pending={selectionPending || undefined} className={`playback-colored ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.fader_pickup_required ? "pickup-required" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${assignmentPending ? "assignment-pending" : ""} ${state.updateArmed ? "update-target" : ""}`} style={cardStyle} key={`${slot}-${playback?.number ?? "empty"}`} onPointerDownCapture={interceptPointer} onClickCapture={interceptClick} onClick={(event: ReactMouseEvent) => { if (isSetContextClick(event.nativeEvent)) openConfiguration(playback, slot); }}>
        {assignmentTarget}
        {configurationTarget}
        {representation}
        {hasFader && <VerticalTouchFader disabled={assignmentPending || !playback} label={playbackFaderLabel(playback)} value={value} accentColor={playback?.color}
          mode={playbackFaderModeFeedback(playback, active)}
          display={playbackFaderDisplay(playback, active, value, server.configuration, server.playbacks?.authoritative_controls, state.blackout)}
          actions={touchActions}
          onChange={(next) => playback && void server.poolPlaybackAction(playback.number, "master", { value: next / 100, surface: "physical" })}/>}
        {!hasFader && touchActionButtons.length > 0 && <footer className={`faderless-playback-actions action-count-${touchActionButtons.length}`}>{touchActionButtons}</footer>}
      </article>;
    })}
  </div>{configuration && <PlaybackConfigurationModal playback={configuration.playback} page={configuration.page} slot={configuration.slot} empty={configuration.empty} onClose={() => setConfiguration(null)}/>}</>;
}

export function emptyConfiguration(page: number, slot: number, buttons: number, hasFader: boolean, cueListId: string): PlaybackDefinition {
  return { number: 0, name: `Playback ${page}.${slot}`, target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go_minus", "go", "flash"], button_count: Math.max(0, Math.min(3, buttons)) as 0 | 1 | 2 | 3, fader: "master", has_fader: hasFader, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false };
}
export function playbackButtonLabel(action: PlaybackDefinition["buttons"][number]) { return ({ go: "GO +", go_minus: "GO −", fast_forward: "FAST +", fast_rewind: "FAST −", select_contents: "SELECT CONTENTS", select_dereferenced: "SELECT FIXTURES", pause_dynamics: "PAUSE DYNAMICS", none: "DISABLED" } as Partial<Record<typeof action, string>>)[action] ?? action.toUpperCase(); }
function isHeldAction(action: PlaybackDefinition["buttons"][number]) { return action === "flash" || action === "swap"; }
function buttonFeedbackClass(action: PlaybackDefinition["buttons"][number], active: PlaybackSnapshotActive | undefined, selected: boolean, blackout: boolean) { const on = action === "select" ? selected : action === "flash" ? Boolean(active?.flash) : action === "temp" ? Boolean(active?.temporary_active) : action === "swap" ? Boolean(active?.swap_active) : action === "pause" ? Boolean(active?.paused) : action === "blackout" ? blackout : action === "on" || action === "toggle" ? Boolean(active?.enabled) : false; return on ? "playback-button-active" : ""; }
type PlaybackSnapshotActive = NonNullable<ReturnType<typeof useServer>["playbacks"]>["active"][number];
type AuthoritativeControls = NonNullable<NonNullable<ReturnType<typeof useServer>["playbacks"]>["authoritative_controls"]>;
function playbackFaderValue(playback: PlaybackDefinition | null, active: PlaybackSnapshotActive | undefined, groupMaster: number | undefined, configuration: ReturnType<typeof useServer>["configuration"], controls: AuthoritativeControls | undefined, grandMaster: number) {
  if (!playback) return 0;
  if (playback.target.type === "group") { const groupId = playback.target.group_id; return Math.round((controls?.groups.find((item) => item.id === groupId)?.master ?? groupMaster ?? 1) * 100); }
  if (playback.target.type === "speed_group") { const speed = controls?.speed_groups[playback.target.group.charCodeAt(0) - 65]; const bpm = speed?.effective_bpm ?? configuration?.speed_groups_bpm[playback.target.group.charCodeAt(0) - 65] ?? 120; return playback.fader === "direct_bpm" ? bpm / 3 : playback.fader === "centered_relative" ? speed ? centeredRelativePosition(speed.speed_master_scale) : 50 : Math.round((speed?.speed_master_scale ?? Math.min(1, bpm / Math.max(1, speed?.manual_bpm ?? 120))) * 100); }
  if (playback.target.type === "programmer_fade") return (controls?.programmer_fade_millis ?? configuration?.programmer_fade_millis ?? 3_000) / 200;
  if (playback.target.type === "cue_fade") return (controls?.cue_fade_millis ?? configuration?.sequence_master_fade_millis ?? 3_000) / 600;
  if (playback.target.type === "grand_master") return Math.round((controls?.grand_master.level ?? grandMaster) * 100);
  if (playback.fader === "x_fade") return Math.round((active?.manual_xfade_position ?? 0) * 100);
  if (playback.fader === "temp") return Math.round((active?.temporary_master ?? 0) * 100);
  return Math.round((active?.fader_position ?? active?.master ?? 0) * 100);
}
function playbackFaderLabel(playback: PlaybackDefinition | null) { if (!playback) return "Empty"; if (playback.target.type === "group") return "Group master"; if (playback.target.type === "speed_group") return `Speed Group ${playback.target.group}`; if (playback.target.type === "programmer_fade") return "Programmer Fade"; if (playback.target.type === "cue_fade") return "Cue Fade"; if (playback.target.type === "grand_master") return "Grand Master"; return playback.fader === "x_fade" ? "X-fade" : playback.fader === "temp" ? "Temp" : "Master"; }
function playbackFaderModeFeedback(playback: PlaybackDefinition | null, active: PlaybackSnapshotActive | undefined) { if (active?.fader_pickup_required) return "Pickup: lower to zero"; if (playback?.fader === "x_fade") return active?.manual_xfade_direction === "towards_low" ? "Travel towards low" : "Travel towards high"; if (playback?.fader === "temp" && active?.temporary_active) return "Temporary active"; return undefined; }
function playbackFaderDisplay(playback: PlaybackDefinition | null, active: PlaybackSnapshotActive | undefined, value: number, configuration: ReturnType<typeof useServer>["configuration"], controls: AuthoritativeControls | undefined, blackout: boolean) {
  if (!playback) return "Empty";
  if (playback.target.type === "speed_group") { const speed = controls?.speed_groups[playback.target.group.charCodeAt(0) - 65]; const bpm = speed?.effective_bpm ?? configuration?.speed_groups_bpm[playback.target.group.charCodeAt(0) - 65] ?? 120; return `${Math.round(bpm)} BPM · ${speed?.paused ? "PAUSED" : speed?.source?.replaceAll("_", " ").toUpperCase() ?? `${value.toFixed(0)}%`}`; }
  if (playback.target.type === "programmer_fade") return `${((controls?.programmer_fade_millis ?? configuration?.programmer_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`;
  if (playback.target.type === "cue_fade") return `${((controls?.cue_fade_millis ?? configuration?.sequence_master_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`;
  if (playback.target.type === "grand_master") { const master = controls?.grand_master; return `${value}%${master?.blackout ?? blackout ? " · BLACKOUT" : ""}${master?.dynamics_paused ? " · DYNAMICS PAUSED" : ""}`; }
  if (playback.target.type === "group") return `${value}% master`;
  if (playback.fader === "x_fade") { const current = active?.current_cue_number ?? (active?.cue_index == null ? "—" : active.cue_index + 1); return `Cue ${current} → ${active?.effective_next_cue_number ?? "—"} · ${Math.round((active?.manual_xfade_progress ?? 0) * 100)}%`; }
  if (playback.fader === "temp") return `${active?.temporary_active ? "TEMP" : "Temp"} · ${value}%`;
  if (active?.loaded_cue_number != null) return `Load ${active.loaded_cue_number} · ${value}%`;
  if (active?.enabled !== false && active) return `Cue ${active.current_cue_number ?? active.cue_index + 1} · ${value}%`;
  return `${value}%`;
}
function centeredRelativePosition(scale: number) { return Math.max(0, Math.min(100, (0.5 + Math.log(Math.max(0.25, Math.min(4, scale))) / Math.log(4) / 2) * 100)); }
