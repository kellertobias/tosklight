import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";
import { Button, Input } from "../common";

type Draft = { ip: string; port: number };

export function MediaServerSetup() {
  const server = useServer();
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [live, setLive] = useState<Set<string>>(() => new Set());
  const mediaFixtures = (server.patch?.fixtures ?? []).filter((fixture) =>
    Boolean(fixture.direct_control) ||
    Boolean(fixture.definition.direct_control_protocols?.length) ||
    (fixture.definition.heads ?? []).some((head) => head.parameters.some((parameter) => parameter.attribute.startsWith("media.")))
  );
  useEffect(() => setDrafts(Object.fromEntries(mediaFixtures.map((fixture) => [fixture.fixture_id, { ip: fixture.direct_control?.ip_address ?? "", port: fixture.direct_control?.port ?? 4811 }]))), [server.patch]);
  useEffect(() => {
    if (!live.size) return;
    const timer = window.setInterval(() => { for (const fixtureId of live) void server.refreshMediaPreview(fixtureId); }, 1_000);
    return () => window.clearInterval(timer);
  }, [live, server]);
  if (!mediaFixtures.length) return <p>No patched devices expose media capabilities.</p>;
  return <div className="media-server-setup"><p>CITP endpoints belong to the physical master fixture. Every logical media layer inherits the same endpoint.</p>{mediaFixtures.map((fixture) => <MediaServerCard key={fixture.fixture_id} fixture={fixture} status={server.mediaServers.find((item) => item.fixture_id === fixture.fixture_id)} draft={drafts[fixture.fixture_id] ?? { ip: "", port: 4811 }} preview={server.mediaPreviewUrls[fixture.fixture_id]} busy={busy === fixture.fixture_id} live={live.has(fixture.fixture_id)} setDraft={(draft) => setDrafts((current) => ({ ...current, [fixture.fixture_id]: draft }))} save={async (draft) => { setBusy(fixture.fixture_id); try { await server.configureMediaServer(fixture.fixture_id, draft.ip.trim() || null, draft.port); } finally { setBusy(null); } }} toggleLive={async () => { if (live.has(fixture.fixture_id)) { setLive((current) => { const next = new Set(current); next.delete(fixture.fixture_id); return next; }); return; } setBusy(fixture.fixture_id); try { if (await server.refreshMediaPreview(fixture.fixture_id)) setLive((current) => new Set(current).add(fixture.fixture_id)); } finally { setBusy(null); } }} refreshThumbnails={async () => { setBusy(fixture.fixture_id); try { await server.refreshMediaThumbnails(fixture.fixture_id, Array.from({ length: 16 }, (_, index) => index)); } finally { setBusy(null); } }}/>)}</div>;
}

function MediaServerCard({ fixture, status, draft, preview, busy, live, setDraft, save, toggleLive, refreshThumbnails }: { fixture: PatchedFixture; status?: MediaServerFixture; draft: Draft; preview?: string; busy: boolean; live: boolean; setDraft: (draft: Draft) => void; save: (draft: Draft) => Promise<void>; toggleLive: () => Promise<void>; refreshThumbnails: () => Promise<void> }) {
  const name = `${fixture.definition.manufacturer} ${fixture.definition.model}`;
  const supportsCitp = fixture.definition.direct_control_protocols?.includes("citp") ?? Boolean(fixture.direct_control);
  const statusText = status?.status.online ? "● Online" : fixture.direct_control ? "● Offline" : supportsCitp ? "Not configured" : "Profile has no CITP capability";
  return <article className="media-server-card">
    <header><b>{name}</b><span className={status?.status.online ? "online" : "offline"}>{statusText}</span></header>
    <div className="media-endpoint-form"><label>IP address<Input disabled={!supportsCitp} aria-label={`${name} CITP IP address`} value={draft.ip} placeholder="192.168.1.50" onChange={(event) => setDraft({ ...draft, ip: event.target.value })}/></label><label>Port<Input disabled={!supportsCitp} aria-label={`${name} CITP port`} type="number" min="1" max="65535" value={draft.port} onChange={(event) => setDraft({ ...draft, port: Number(event.target.value) })}/></label><Button disabled={!supportsCitp || busy} onClick={() => void save(draft)}>{draft.ip.trim() ? "Save endpoint" : "Disable CITP"}</Button></div>
    {fixture.direct_control && <div className="media-actions"><Button className={live ? "active" : ""} disabled={busy} onClick={() => void toggleLive()}>{live ? "Stop live preview" : "Start live preview"}</Button><Button disabled={busy} onClick={() => void refreshThumbnails()}>Refresh thumbnails 1–16</Button></div>}
    {preview ? <img className="media-preview" src={preview} alt={`${name} live CITP output preview`}/> : <div className="media-preview media-preview-empty">{status?.status.last_error ? <><b>Preview unavailable</b><small>{status.status.last_error}</small></> : "No cached preview"}</div>}
    <small>{fixture.logical_heads.length} logical layers · {status?.status.last_success ? `Last response ${new Date(status.status.last_success).toLocaleString()}` : "No successful response yet"}</small>
  </article>;
}
