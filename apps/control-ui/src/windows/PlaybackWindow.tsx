import { useEffect, useMemo, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { AttributeValue, VisualizationSnapshot } from "../api/types";
import { cueVisualization, migrateStagePosition, renderStageThumbnail } from "./stage3dScene";

export function PlaybackWindow({ compact, playbackTab }: WindowProps) {
  const server = useServer();
  const [tab, setTab] = useState<"pool" | "cues">(playbackTab ?? "pool");
  const [selectedPlayback, setSelectedPlayback] = useState<number>(1);
  const [search, setSearch] = useState("");
  const selectedDefinition = server.playbacks?.pool.find((playback) => playback.number === selectedPlayback);
  const selectedCueListId = selectedDefinition?.target.type === "cue_list" ? selectedDefinition.target.cue_list_id : null;
  const sequence = selectedCueListId ? server.playbacks?.cue_lists.find((cue) => cue.id === selectedCueListId) : server.playbacks?.cue_lists[0];
  const active = sequence && server.playbacks?.active.find((item) => item.cue_list_id === sequence.id);
  const cues = sequence?.cues ?? [];
  const [selectedCue, setSelectedCue] = useState(0);
  const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  const stageFixtures = useMemo(() => (server.patch?.fixtures ?? []).map((fixture, index) => ({ fixture, index, position: server.stageLayout?.body.positions3d?.[fixture.fixture_id] ?? migrateStagePosition(server.stageLayout?.body.positions[fixture.fixture_id], index) })), [server.patch, server.stageLayout]);
  useEffect(() => {
    if (!tauri || !cues.length || !stageFixtures.length) return;
    let cancelled = false;
    void server.readVisualization().then((live) => {
      if (cancelled) return;
      let state: VisualizationSnapshot = { ...live, values: [] };
      const next: Record<number, string> = {};
      for (let index = 0; index < cues.length; index++) {
        const changes = [...(cues[index].changes ?? [])] as Array<{ fixture_id: string; attribute: string; value: AttributeValue | null }>;
        for (const groupChange of cues[index].group_changes ?? []) {
          const group = server.groups.find((candidate) => candidate.id === groupChange.group_id);
          for (const fixture_id of group?.body.fixtures ?? []) changes.push({ fixture_id, attribute: groupChange.attribute, value: groupChange.value });
        }
        state = cueVisualization(state, changes);
        next[index] = renderStageThumbnail(stageFixtures, state);
      }
      if (!cancelled) setThumbnails(next);
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [tauri, cues, stageFixtures, server.groups, server.readVisualization]);
  const tabs = <nav className="playback-window-tabs"><button className={tab === "pool" ? "active" : ""} onClick={() => setTab("pool")}>Playback Pool</button><button className={tab === "cues" ? "active" : ""} onClick={() => setTab("cues")}>Cue List</button></nav>;
  if (tab === "pool") return <div className="playback-window"><header className="window-toolbar"><h1>Playback Pool <small>{server.playbacks?.pool.length ?? 0} / 1000 assigned</small></h1><span className="spacer"/><input aria-label="Search playbacks" placeholder="Number or name" value={search} onChange={(event) => setSearch(event.target.value)}/></header>{tabs}<div className="playback-pool-layout"><div className="playback-pool-grid">{(server.playbacks?.pool ?? []).filter((playback) => !search || playback.name.toLowerCase().includes(search.toLowerCase()) || String(playback.number).includes(search)).map((playback) => { const runtime = server.playbacks?.active.find((item) => item.playback_number === playback.number); const usage = (server.playbacks?.pages ?? []).filter((page) => Object.values(page.slots).includes(playback.number)).map((page) => page.number); return <button key={playback.number} className={`${runtime ? "running" : ""} ${selectedPlayback === playback.number ? "selected" : ""}`} onClick={() => setSelectedPlayback(playback.number)}><strong>{playback.number}</strong><span>{playback.name}</span><small>{playback.target.type === "cue_list" ? "Cue list" : "Group"} · {runtime ? `${Math.round(runtime.master * 100)}%` : "Off"}</small><small>{usage.length ? `Pages ${usage.join(", ")}` : "Not on a page"}</small></button>; })}</div>{selectedDefinition && <aside className="playback-pool-settings"><h3>{selectedDefinition.number} · {selectedDefinition.name}</h3>{selectedDefinition.target.type === "cue_list" && <button onClick={() => setTab("cues")}>Open cue list</button>}<label><input type="checkbox" checked={selectedDefinition.go_activates} onChange={(event) => void server.savePlaybackDefinition({ ...selectedDefinition, go_activates: event.target.checked })}/> Go turns playback on</label><label><input type="checkbox" checked={selectedDefinition.auto_off} onChange={(event) => void server.savePlaybackDefinition({ ...selectedDefinition, auto_off: event.target.checked })}/> Switch off when fully overwritten</label><label>X-fade milliseconds<input type="number" min="0" max="60000" value={selectedDefinition.xfade_millis} onChange={(event) => void server.savePlaybackDefinition({ ...selectedDefinition, xfade_millis: Number(event.target.value) })}/></label><label>Fader<select value={selectedDefinition.fader} onChange={(event) => void server.savePlaybackDefinition({ ...selectedDefinition, fader: event.target.value as typeof selectedDefinition.fader })}><option value="master">Master</option><option value="temp">Temp</option><option value="speed" disabled>Speed (future)</option></select></label>{selectedDefinition.buttons.map((action, index) => <label key={index}>Button {index + 1}<select value={action} onChange={(event) => { const buttons = [...selectedDefinition.buttons] as typeof selectedDefinition.buttons; buttons[index] = event.target.value as typeof action; void server.savePlaybackDefinition({ ...selectedDefinition, buttons }); }}>{["none","on","off","toggle","go","go_minus","flash"].map((value) => <option key={value} value={value}>{value.replace("_", " ")}</option>)}</select></label>)}</aside>}</div></div>;
  return <div className="playback-window"><header className="window-toolbar"><h1>Sequence · {sequence?.name ?? "No sequence"} <small>{active ? "Running" : "Ready"} · revision {server.patch?.revision ?? 0}</small></h1><span className="spacer"/>{sequence && <span>{sequence.mode} · priority {sequence.priority}</span>}</header>{tabs}<div className="sequence-layout"><div className="cue-editor"><div className="cue-list">{cues.length === 0 && <div className="empty-window-message">No cue list is available in the active show.</div>}{cues.map((cue, index) => <button onClick={() => setSelectedCue(index)} key={cue.number} className={`cue-row ${thumbnails[index] ? "with-thumbnail" : ""} ${active?.cue_index === index ? "current" : active?.cue_index === index - 1 ? "next" : ""} ${selectedCue === index ? "selected" : ""}`}>{thumbnails[index] && <img src={thumbnails[index]} alt=""/>}<b>{cue.number}</b><span>{cue.name || `Cue ${cue.number}`}</span><span>{cue.trigger.type}</span><span>{(cue.fade_millis / 1000).toFixed(1)} s</span><span>{index === active?.cue_index ? "Active" : "Tracked"}</span></button>)}</div></div>{!compact && <aside className="sequence-actions"><button className="go" disabled={!selectedDefinition} onClick={() => selectedDefinition && void server.poolPlaybackAction(selectedDefinition.number, "go")}>GO<br/><small>{sequence ? `Next · ${cues[(active?.cue_index ?? -1) + 1]?.name ?? "End"}` : "No sequence"}</small></button><button disabled={!selectedDefinition} onClick={() => selectedDefinition && void server.poolPlaybackAction(selectedDefinition.number, "go-minus")}>GO −</button><button disabled={!selectedDefinition} onClick={() => selectedDefinition && void server.poolPlaybackAction(selectedDefinition.number, "toggle")}>TOGGLE</button><button disabled={!selectedDefinition} onClick={() => selectedDefinition && void server.poolPlaybackAction(selectedDefinition.number, "off")}>OFF</button>{sequence && cues[selectedCue] && <section>{thumbnails[selectedCue] && <img className="cue-selected-thumbnail" src={thumbnails[selectedCue]} alt={`3D preview for cue ${cues[selectedCue].number}`}/>}<b>Selected cue · {cues[selectedCue].number}</b><p>{cues[selectedCue].name || "Unnamed cue"}</p><small>Fade {(cues[selectedCue].fade_millis / 1000).toFixed(1)} s · Delay {(cues[selectedCue].delay_millis / 1000).toFixed(1)} s</small></section>}</aside>}</div></div>;
}
