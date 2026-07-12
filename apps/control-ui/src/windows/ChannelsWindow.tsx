import { useState } from "react";
import { fixtures } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";

export function ChannelsWindow({ compact }: WindowProps) {
  const server = useServer();
  const [page, setPage] = useState(0);
  const patched = server.patch?.fixtures.map((item, index) => ({ ...fixtures[index % fixtures.length], id: index + 1, name: item.definition.name ?? item.definition.model, fixtureId: item.fixture_id })) ?? [];
  const channels = server.bootstrap ? patched : fixtures.map((fixture) => ({ ...fixture, fixtureId: "" }));
  const pageSize = compact ? 10 : 20;
  const pages = Math.max(1, Math.ceil(channels.length / pageSize));
  const visible = channels.slice(page * pageSize, page * pageSize + pageSize);
  return <div className="channels-window"><header className="window-toolbar"><h1>Channels · Intensity <small>Channels select their fixtures</small></h1><span className="spacer"/>{!compact && <><button aria-label="Previous channel page" disabled={page === 0} onClick={() => setPage(page - 1)}>←</button><span>{page * pageSize + 1}–{Math.min(channels.length, (page + 1) * pageSize)} / {channels.length}</span><button aria-label="Next channel page" disabled={page >= pages - 1} onClick={() => setPage(page + 1)}>→</button></>}</header><div className="channel-bank">{channels.length === 0 && <div className="empty-window-message">No fixture channels are available in the active patch.</div>}{visible.map((fixture) => <article className={`channel-strip ${server.selectedFixtures.includes(fixture.fixtureId) ? "selected" : ""}`} key={fixture.fixtureId || fixture.id}><b>CH {fixture.id}</b><div className="channel-meter"><i style={{ height: `${fixture.dimmer}%` }}/><strong>{fixture.dimmer}%</strong></div><small>{fixture.name}</small><button disabled={!fixture.fixtureId} onClick={() => fixture.fixtureId && void server.setSelection([fixture.fixtureId])}>SELECT</button></article>)}</div></div>;
}
