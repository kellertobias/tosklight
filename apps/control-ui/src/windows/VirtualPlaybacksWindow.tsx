import { useApp } from "../state/AppContext";
import { useServer } from "../api/ServerContext";
import { Button } from "../components/common";
import type { WindowProps } from "./windowTypes";

export function VirtualPlaybacksWindow({ paneId }: WindowProps) {
  const { state } = useApp();
  const server = useServer();
  const pane = state.desks.flatMap((desk) => desk.panes).find((candidate) => candidate.id === paneId);
  const rows = pane?.virtualPlaybackRows ?? 2;
  const columns = pane?.virtualPlaybackColumns ?? 2;
  const cells = pane?.virtualPlaybackCells ?? [];
  return <div className="virtual-playback-grid" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))` }}>
    {Array.from({ length: rows * columns }, (_, index) => {
      const cell = cells[index] ?? { playbackNumber: null, action: "go" as const };
      const playback = server.playbacks?.pool.find((candidate) => candidate.number === cell.playbackNumber);
      const runtime = server.playbacks?.active.find((candidate) => candidate.playback_number === cell.playbackNumber);
      return <Button key={index} className={`virtual-playback-cell ${runtime ? "active" : ""}`} disabled={!playback} onClick={() => playback && void server.poolPlaybackAction(playback.number, cell.action, { surface: "virtual" })}>
        <span>{index + 1}</span><b>{playback?.name ?? "Unassigned"}</b><small>{cell.action.toUpperCase()}{runtime ? ` · Cue ${(runtime.cue_index ?? 0) + 1}` : ""}</small>
      </Button>;
    })}
  </div>;
}
