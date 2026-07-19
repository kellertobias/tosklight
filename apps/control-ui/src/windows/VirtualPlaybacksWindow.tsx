import { useEffect, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { useApp } from "../state/AppContext";
import { useServer } from "../api/ServerContext";
import { Button, FormLayout, TextField } from "../components/common";
import { emptyConfiguration } from "../components/control/PlaybackFaderBank";
import { normalizePlaybackTopology, PlaybackConfigurationModal } from "../components/control/PlaybackConfigurationModal";
import type { PlaybackDefinition } from "../api/types";
import type { VirtualPlaybackExclusionZone } from "../types";
import type { WindowProps } from "./windowTypes";
import { cueUpdateTarget, requestUpdateTarget } from "../components/control/updateWorkflow";
import {
	usePlaybackDeskView,
	usePlaybackProjectionMap,
} from "../features/playbackRuntime/PlaybackRuntimeView";
import { legacyPlaybackRuntime } from "../features/playbackRuntime/legacy";

export function VirtualPlaybacksWindow({ paneId, active = true }: WindowProps) {
  const { state, dispatch } = useApp();
  const server = useServer();
  const surfaceId = paneId ?? "builtin-virtual-playbacks";
  const pane = state.desks.flatMap((desk) => desk.panes).find((candidate) => candidate.id === paneId);
  const rows = pane?.virtualPlaybackRows ?? 2;
  const columns = pane?.virtualPlaybackColumns ?? 2;
  const playbackDesk = usePlaybackDeskView(active);
  const pageNumber = playbackDesk?.active_page ?? server.playbacks?.active_page ?? state.playbackPage + 1;
  const page = server.playbacks?.pages.find((candidate) => candidate.number === pageNumber);
  const playbackNumbers = Array.from({ length: rows * columns }, (_, index) => page?.slots[String(index + 1)]).filter((number): number is number => number != null);
  const runtimeByPlayback = usePlaybackProjectionMap(active ? playbackNumbers : []);
  const [configuration, setConfiguration] = useState<{ playback: PlaybackDefinition; slot: number; empty: boolean } | null>(null);
  const [selectedSlots, setSelectedSlots] = useState<number[]>([]);
  const [creatingZone, setCreatingZone] = useState(false);
  const [zoneName, setZoneName] = useState("");
  const [builtInZones, setBuiltInZones] = useState<VirtualPlaybackExclusionZone[]>([]);
  const zones = pane ? pane.virtualPlaybackExclusionZones ?? [] : builtInZones;
  const configurationArmed = state.playbackSetArmed || (state.cueListSetArmed && state.cueListSetTarget == null);
  const assignmentPending = state.cueListSetTarget != null;

  useEffect(() => {
    let cancelled = false;
    void server.readVirtualPlaybackExclusionZones()
      .then((snapshot) => {
        if (cancelled) return;
        const restored = snapshot.surfaces[surfaceId] ?? [];
        if (paneId) dispatch({ type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES", id: paneId, zones: restored });
        else setBuiltInZones(restored);
      })
      .catch(() => undefined);
    return () => { cancelled = true; };
    // Surface configuration is scoped by the authenticated desk and active show.
  }, [paneId, server.session?.desk.id, server.bootstrap?.active_show?.id]);

  useEffect(() => setSelectedSlots((current) => current.filter((slot) => slot <= rows * columns)), [rows, columns]);

  const openConfiguration = (playback: PlaybackDefinition | null, slot: number) => {
    const next = playback ?? emptyConfiguration(pageNumber, slot, 1, false, server.playbacks?.cue_lists[0]?.id ?? "");
    setConfiguration({ playback: normalizePlaybackTopology({ ...next, button_count: 1, has_fader: false }, 1, false), slot, empty: !playback });
    dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
    dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
    dispatch({ type: "SET_SHIFT_ARMED", value: false });
  };
  const assignSource = async (slot: number) => {
    if (state.cueListSetTarget == null) return;
    const source = server.playbacks?.pool.find((candidate) => candidate.number === state.cueListSetTarget);
    if (!source || source.target.type !== "cue_list") return;
    const draft = emptyConfiguration(pageNumber, slot, 1, false, source.target.cue_list_id);
    const saved = await server.savePlaybackSlot(pageNumber, slot, { ...draft, name: source.name, color: source.color, buttons: [source.buttons[0] === "none" ? "go" : source.buttons[0], "none", "none"], presentation_icon: source.presentation_icon, presentation_image: source.presentation_image });
    if (saved) dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
  };
  const toggleZoneSlot = (slot: number) => setSelectedSlots((current) => current.includes(slot) ? current.filter((candidate) => candidate !== slot) : [...current, slot].sort((left, right) => left - right));
  const createZone = async () => {
    const name = zoneName.trim();
    if (!name || selectedSlots.length < 2) return;
    const zone: VirtualPlaybackExclusionZone = { id: crypto.randomUUID(), name, slots: [...selectedSlots] };
    const next = [...zones, zone];
    if (!await server.saveVirtualPlaybackExclusionZones(surfaceId, next)) return;
    if (paneId) dispatch({ type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES", id: paneId, zones: next });
    else setBuiltInZones(next);
    dispatch({ type: "SET_SHIFT_ARMED", value: false });
    setSelectedSlots([]);
    setZoneName("");
    setCreatingZone(false);
  };

  return <section className="virtual-playback-pane" aria-label={`Virtual Playbacks page ${pageNumber}`}>
    <header className="virtual-playback-toolbar"><Button onClick={() => { dispatch({ type: "SET_CUELIST_SET_TARGET", value: null }); dispatch({ type: "SET_CUELIST_SET_ARMED", value: true }); }}>Set Source</Button><Button onClick={() => dispatch({ type: "SET_CUELIST_SET_ARMED", value: true })}>Add Target</Button>{selectedSlots.length >= 2 && <Button className="primary" onClick={() => { setZoneName(`Exclusion Zone ${zones.length + 1}`); setCreatingZone(true); }}>Create Exclusion Zone</Button>}{selectedSlots.length > 0 && <Button onClick={() => { setSelectedSlots([]); dispatch({ type: "SET_SHIFT_ARMED", value: false }); }}>Cancel zone selection</Button>}<span>{selectedSlots.length > 0 ? `${selectedSlots.length} cells selected · ` : ""}Page {pageNumber} · {rows}×{columns}</span></header>
    <div className="virtual-playback-grid" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))` }}>
      {Array.from({ length: rows * columns }, (_, index) => {
        const slot = index + 1;
        const number = page?.slots[String(slot)];
        const playback = server.playbacks?.pool.find((candidate) => candidate.number === number) ?? null;
        const runtime = legacyPlaybackRuntime(playback ? runtimeByPlayback.get(playback.number) : undefined) ?? (playback ? server.playbacks?.active.find((candidate) => candidate.playback_number === playback.number) : undefined);
        const playbackCueListId = playback?.target.type === "cue_list" ? playback.target.cue_list_id : null;
        const cueList = playbackCueListId ? server.playbacks?.cue_lists.find((candidate) => candidate.id === playbackCueListId) : null;
        const currentCue = cueList && runtime && runtime.cue_index >= 0 ? cueList.cues[runtime.cue_index] : null;
        const requestPlaybackUpdate = () => {
          if (!playback || playback.target.type !== "cue_list") return;
          requestUpdateTarget(cueUpdateTarget(playback.target.cue_list_id, playback.number, currentCue?.id ? { id: currentCue.id, number: currentCue.number } : null));
        };
        const action = playback?.buttons[0] ?? "none";
        const held = action === "flash" || action === "swap";
        const background = playback?.presentation_image ? `linear-gradient(#08101488,#081014cc),url(${JSON.stringify(playback.presentation_image)})` : undefined;
        const style = playback ? { "--playback-color": playback.color ?? "#20c997", backgroundImage: background } as CSSProperties : undefined;
        const selectedForZone = selectedSlots.includes(slot);
        const containingZones = zones.filter((zone) => zone.slots.includes(slot));
        const intercept = (event: ReactPointerEvent<HTMLButtonElement>) => { if (state.updateArmed) { event.preventDefault(); event.stopPropagation(); return true; } if (state.shiftArmed || event.shiftKey) { event.preventDefault(); event.stopPropagation(); return true; } if (!configurationArmed) return false; event.preventDefault(); event.stopPropagation(); openConfiguration(playback, slot); return true; };
        return <Button key={slot} aria-label={`Virtual playback page ${pageNumber} cell ${slot}${playback ? ` ${playback.name}` : " empty"}`} aria-pressed={selectedForZone} data-exclusion-zones={containingZones.map((zone) => zone.name).join(", ")} className={`virtual-playback-cell ${playback ? "playback-colored" : ""} ${runtime?.enabled !== false && runtime ? "running" : ""} ${configurationArmed ? "configuration-armed" : ""} ${assignmentPending ? "assignment-pending" : ""} ${selectedForZone ? "exclusion-selected" : ""} ${containingZones.length > 0 ? "exclusion-member" : ""} ${state.updateArmed ? "update-target" : ""}`} style={style}
          onPointerDown={(event) => { if (intercept(event)) return; if (assignmentPending) { event.preventDefault(); return; } if (playback && held) { event.currentTarget.setPointerCapture?.(event.pointerId); void server.poolPlaybackAction(playback.number, "button", { button: 1, pressed: true, surface: "virtual" }); } }}
          onPointerUp={(event) => !state.updateArmed && !(state.shiftArmed || event.shiftKey) && playback && held && void server.poolPlaybackAction(playback.number, "button", { button: 1, pressed: false, surface: "virtual" })}
          onPointerCancel={(event) => !state.updateArmed && !(state.shiftArmed || event.shiftKey) && playback && held && void server.poolPlaybackAction(playback.number, "button", { button: 1, pressed: false, surface: "virtual" })}
          onLostPointerCapture={(event) => !state.updateArmed && !(state.shiftArmed || event.shiftKey) && playback && held && void server.poolPlaybackAction(playback.number, "button", { button: 1, pressed: false, surface: "virtual" })}
          onClick={(event) => { if (state.updateArmed) { event.preventDefault(); requestPlaybackUpdate(); return; } if (state.shiftArmed || event.shiftKey) { event.preventDefault(); toggleZoneSlot(slot); return; } if (configurationArmed) { event.preventDefault(); openConfiguration(playback, slot); return; } if (assignmentPending) { void assignSource(slot); return; } if (playback && !held && action !== "none") void server.poolPlaybackAction(playback.number, "button", { button: 1, pressed: true, surface: "virtual" }); }}>
          <span>{playback?.presentation_icon ?? slot}</span><b>{playback?.name ?? "Empty"}</b><small>{selectedForZone ? "Selected for exclusion zone" : assignmentPending ? `Assign Cuelist ${state.cueListSetTarget}` : configurationArmed ? "Configure Playback" : containingZones.length > 0 ? containingZones.map((zone) => zone.name).join(" · ") : playback ? `${action.replaceAll("_", " ").toUpperCase()}${runtime ? ` · Cue ${(runtime.cue_index ?? 0) + 1}` : ""}` : "Unassigned"}</small>
        </Button>;
      })}
    </div>
    {configuration && <PlaybackConfigurationModal playback={configuration.playback} page={pageNumber} slot={configuration.slot} empty={configuration.empty} virtual onClose={() => setConfiguration(null)}/>}
    {creatingZone && <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setCreatingZone(false)}><section className="nested-modal virtual-playback-zone-modal" role="dialog" aria-modal="true" aria-label="Create Exclusion Zone"><Button className="modal-close" onClick={() => setCreatingZone(false)}>×</Button><h3>Create Exclusion Zone</h3><p>Cells {selectedSlots.join(", ")} on the current page will be mutually exclusive. Creating the zone does not operate any playback.</p><FormLayout labelPlacement="side"><TextField label="Zone name" autoFocus maxLength={80} value={zoneName} onChange={(event) => setZoneName(event.target.value)}/></FormLayout><footer><Button onClick={() => setCreatingZone(false)}>Cancel</Button><Button className="primary" disabled={!zoneName.trim() || selectedSlots.length < 2} onClick={() => void createZone()}>Create zone</Button></footer>{server.error && <p className="modal-error">{server.error}</p>}</section></div>}
  </section>;
}
