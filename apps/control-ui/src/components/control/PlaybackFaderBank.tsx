import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { useApp } from "../../state/AppContext";
import { playbackSlotNumbers } from "./playbackProjection";
import { Button } from "../common";
import type { CSSProperties } from "react";

export interface PlaybackFaderBankProps { pageNumber?: number; firstSlot?: number; count?: number; rows?: number; buttons?: number }
export function PlaybackFaderBank({ pageNumber, firstSlot = 1, count, rows, buttons }: PlaybackFaderBankProps = {}) {
  const server = useServer();
  const { state } = useApp();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const pageSize = count ?? state.playbackColumns * state.playbackRows;
  const rowCount = rows ?? state.playbackRows;
  const columns = Math.ceil(pageSize / rowCount);
  const page = server.playbacks?.pages.find((candidate) => candidate.number === (pageNumber ?? server.playbacks?.active_page ?? state.playbackPage + 1));
  const slots = playbackSlotNumbers(page, firstSlot, pageSize).map((number) => {
    const playback = server.playbacks?.pool.find((candidate) => candidate.number === number) ?? null;
    const cueListId = playback?.target.type === "cue_list" ? playback.target.cue_list_id : null;
    const groupId = playback?.target.type === "group" ? playback.target.group_id : null;
    const cue = cueListId ? server.playbacks?.cue_lists.find((candidate) => candidate.id === cueListId) ?? null : null;
    const group = groupId ? server.groups.find((candidate) => candidate.id === groupId) ?? null : null;
    return { playback, cue, group };
  });
  return <div className="playback-fader-bank" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${rowCount}, minmax(0, 1fr))` }}>
    {slots.map(({ playback, cue, group }, index) => {
      const active = playback ? server.playbacks?.active.find((item) => item.playback_number === playback.number) : undefined;
      const value = group ? Math.round((group.body.master ?? 0) * 100) : Math.round((active?.master ?? 0) * 100);
      const actions = (playback?.buttons ?? ["none", "none", "none"]).slice(0, hardware ? 2 : buttons ?? server.playbacks?.desk.buttons ?? 3);
      const actionButtons = actions.map((action, button) => <Button key={button} disabled={!playback || action === "none"}
        onClick={() => playback && action !== "flash" && action !== "none" && void server.poolPlaybackAction(playback.number, action === "go_minus" ? "go-minus" : action)}
        onPointerDown={(event) => { if (!playback || action !== "flash") return; event.currentTarget.setPointerCapture(event.pointerId); void server.poolPlaybackAction(playback.number, "flash", { pressed: true }); }}
        onPointerUp={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}
        onPointerCancel={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}
        onLostPointerCapture={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}>
        {action === "go_minus" ? "GO −" : action.toUpperCase()}
      </Button>);
      if (hardware) {
        const cueIndex = active?.cue_index ?? -1;
        const cueName = (offset: number) => cue?.cues[cueIndex + offset]?.name || (cue?.cues[cueIndex + offset] ? `Cue ${cue.cues[cueIndex + offset].number}` : "—");
        return <article className={`hardware-playback-card ${active ? "running" : ""} ${group ? "group-master-playback" : ""} ${!playback ? "empty" : ""}`} key={playback?.number ?? `empty-${index}`}>
          <header><b>{firstSlot + index} · {playback?.name ?? "—"}</b>{cue && <div className="hardware-cue-context"><small>Prev · {cueName(-1)}</small><strong>Now · {cueName(0)}</strong><small>Next · {cueName(1)}</small></div>}</header>
          <div className="hardware-playback-controls"><footer>{actionButtons}</footer><div className="hardware-fader" style={{ "--hardware-fader-level": `${value}%` } as CSSProperties}><i/><b>{value}%</b></div></div>
        </article>;
      }
      return <article className={`${active ? "running" : ""} ${group ? "group-master-playback" : ""} ${!playback ? "empty" : ""}`} key={playback?.number ?? `empty-${index}`}>
        <b>{firstSlot + index} · {playback?.name ?? "—"}</b>
        <VerticalTouchFader disabled={!playback || playback.fader === "speed"} label={playback?.fader ?? "Empty"} value={value}
          display={active && cue ? `Cue ${active.cue_index + 1} · ${value}%` : playback ? `${value}%` : "Empty"}
          onChange={(next) => playback && void server.poolPlaybackAction(playback.number, "master", { value: next / 100 })}/>
        <footer>{actionButtons}</footer>
      </article>;
    })}
  </div>;
}
