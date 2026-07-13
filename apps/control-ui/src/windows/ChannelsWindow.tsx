import { useEffect, useState } from "react";
import type { VisualizationSnapshot } from "../api/types";
import { useServer } from "../api/ServerContext";
import { VerticalTouchFader } from "../components/control/VerticalTouchFader";
import type { WindowProps } from "./windowTypes";
import { fixtureValue } from "./fixtureVisualization";
import { createPortal } from "react-dom";
import { Button } from "../components/common";

const PAGE_SIZE = 20;

export function ChannelsWindow({ compact }: WindowProps) {
  const server = useServer();
  const [page, setPage] = useState(0);
  const [pagePickerOpen, setPagePickerOpen] = useState(false);
  const [visualization, setVisualization] = useState<VisualizationSnapshot | null>(null);
  useEffect(() => {
    let cancelled = false;
    const refresh = () => void server.readVisualization().then((next) => { if (!cancelled) setVisualization(next); }).catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, 250);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [server.readVisualization]);
  useEffect(() => {
    if (!pagePickerOpen) return;
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); setPagePickerOpen(false); } };
    window.addEventListener("keydown", close, true);
    return () => window.removeEventListener("keydown", close, true);
  }, [pagePickerOpen]);
  const channels = server.patch?.fixtures.map((fixture, index) => ({
    number: index + 1,
    fixture,
    name: fixture.definition.name ?? fixture.definition.model,
    level: Math.round(fixtureValue(visualization, fixture, "intensity") * 100),
  })) ?? [];
  const pages = Math.max(8, Math.ceil(channels.length / PAGE_SIZE));
  const visible = Array.from({ length: PAGE_SIZE }, (_, index) => channels[page * PAGE_SIZE + index] ?? null);
  return <div className="channels-window">
    <header className="window-toolbar"><h1>Channels · Intensity <small>Two-row channel bank</small></h1><span className="spacer" />{!compact && <div className="channel-page-controls"><Button aria-label="Previous channel page" disabled={page === 0} onClick={() => setPage(page - 1)}>←</Button><Button className="channel-page-current" onClick={() => setPagePickerOpen(true)}>{page * PAGE_SIZE + 1}–{(page + 1) * PAGE_SIZE}</Button><Button aria-label="Next channel page" disabled={page >= pages - 1} onClick={() => setPage(page + 1)}>→</Button></div>}</header>
    <div className="channel-fader-bank">{visible.map((channel, index) => {
      const number = page * PAGE_SIZE + index + 1;
      return <article className={`channel-fader ${channel ? "" : "empty"} ${channel && server.selectedFixtures.includes(channel.fixture.fixture_id) ? "selected" : ""}`} key={channel?.fixture.fixture_id ?? `empty-${number}`} onClick={() => channel && void server.setSelection([channel.fixture.fixture_id])}>
        <VerticalTouchFader
          disabled={!channel}
          label={channel ? `CH ${number}` : `CH ${number} · Empty`}
          mode={channel?.name ?? "Unpatched"}
          value={channel?.level ?? 0}
          display={channel ? `${channel.level}%` : "—"}
          onChange={(value) => channel && void server.setProgrammer(channel.fixture.fixture_id, "intensity", value / 100)}
        />
      </article>;
    })}</div>
    {pagePickerOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setPagePickerOpen(false)}><div className="nested-modal channel-page-modal" role="dialog" aria-modal="true" aria-label="Channel pages"><Button className="modal-close" onClick={() => setPagePickerOpen(false)}>×</Button><h3>Channel pages</h3><div>{Array.from({ length: pages }, (_, nextPage) => <Button className={nextPage === page ? "active" : ""} key={nextPage} onClick={() => { setPage(nextPage); setPagePickerOpen(false); }}>{nextPage * PAGE_SIZE + 1}–{(nextPage + 1) * PAGE_SIZE}</Button>)}</div></div></div>, document.body)}
  </div>;
}
