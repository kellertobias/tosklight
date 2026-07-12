import { useServer } from "../../api/ServerContext";
import { VerticalTouchFader } from "./VerticalTouchFader";
import { useApp } from "../../state/AppContext";
import { playbackSlotNumbers } from "./playbackProjection";

export interface PlaybackFaderBankProps { pageNumber?: number; firstSlot?: number; count?: number; rows?: number; buttons?: number }
export function PlaybackFaderBank({ pageNumber, firstSlot = 1, count, rows, buttons }: PlaybackFaderBankProps = {}) {
  const server = useServer();
  const { state } = useApp();
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
      return <article className={`${active ? "running" : ""} ${group ? "group-master-playback" : ""} ${!playback ? "empty" : ""}`} key={playback?.number ?? `empty-${index}`}>
        <b>{firstSlot + index} · {playback?.name ?? "—"}</b>
        <VerticalTouchFader disabled={!playback || playback.fader === "speed"} label={playback?.fader ?? "Empty"} value={value}
          display={active && cue ? `Cue ${active.cue_index + 1} · ${value}%` : playback ? `${value}%` : "Empty"}
          onChange={(next) => playback && void server.poolPlaybackAction(playback.number, "master", { value: next / 100 })}/>
        <footer>{(playback?.buttons ?? ["none", "none", "none"]).slice(0, buttons ?? server.playbacks?.desk.buttons ?? 3).map((action, button) => <button key={button} disabled={!playback || action === "none"}
          onClick={() => playback && action !== "flash" && action !== "none" && void server.poolPlaybackAction(playback.number, action === "go_minus" ? "go-minus" : action)}
          onPointerDown={(event) => { if (!playback || action !== "flash") return; event.currentTarget.setPointerCapture(event.pointerId); void server.poolPlaybackAction(playback.number, "flash", { pressed: true }); }}
          onPointerUp={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}
          onPointerCancel={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}
          onLostPointerCapture={() => playback && action === "flash" && void server.poolPlaybackAction(playback.number, "flash", { pressed: false })}>
          {action === "go_minus" ? "GO −" : action.toUpperCase()}
        </button>)}</footer>
      </article>;
    })}
  </div>;
}
