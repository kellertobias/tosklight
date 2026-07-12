import { useRef, useState } from "react";
import { groups as fallbackGroups } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { StoredGroup, VersionedObject } from "../api/types";

export function GroupsWindow({ compact }: WindowProps) {
  const server = useServer();
  const [contextGroup, setContextGroup] = useState<string | null>(null);
  const [nth, setNth] = useState(3);
  const [offset, setOffset] = useState(0);
  const hold = useRef<number | null>(null);
  const fallback = (server.bootstrap ? [] : fallbackGroups.map((group) => ({ kind: "group", id: String(group.id), revision: 0, updated_at: "", body: { name: group.name, fixtures: Array.from({ length: group.fixtures }, (_, index) => String(index)), master: 1, playback_fader: group.id <= 8 ? group.id : null, programming: {}, derived_from: null, frozen_from: null } }))) as typeof server.groups;
  const stored = server.bootstrap?.active_show ? server.groups : fallback;
  const cards = Array.from({ length: compact ? 20 : 40 }, (_, index) => stored.find((group) => group.id === String(index + 1)) ?? null);
  const patched = new Set(server.patch?.fixtures.flatMap((fixture) => [fixture.fixture_id, ...fixture.logical_heads.map((head) => head.fixture_id)]) ?? []);
  const fixtureNames = new Map<string, string>();
  const capabilities = new Map<string, Set<string>>();
  for (const fixture of server.patch?.fixtures ?? []) {
    fixtureNames.set(fixture.fixture_id, `${fixture.definition.manufacturer} ${fixture.definition.model}`);
    for (const head of fixture.definition.heads ?? []) {
      const owner = head.shared ? fixture.fixture_id : fixture.logical_heads.find((candidate) => candidate.head_index === head.index)?.fixture_id;
      if (owner) {
        fixtureNames.set(owner, head.shared ? fixtureNames.get(fixture.fixture_id)! : `${fixtureNames.get(fixture.fixture_id)} · head ${head.index}`);
        capabilities.set(owner, new Set(head.parameters.map((parameter) => parameter.attribute)));
      }
    }
  }
  const contextual = stored.find((group) => group.id === contextGroup);
  const macro = (rule: Record<string, unknown>) => void server.selectionMacro(rule);
  const cancelHold = () => { if (hold.current) window.clearTimeout(hold.current); hold.current = null; };

  return <div className="pool-window group-pool-window">
    <header className="window-toolbar"><h1>Group Pool {!compact && <small>{server.selectedFixtures.length} fixtures selected · ordered</small>}</h1><span className="spacer"/>{!compact && <div className="selection-macros"><button onClick={() => macro({ type: "odd" })}>Odd</button><button onClick={() => macro({ type: "even" })}>Even</button><label>Every <input aria-label="Every Nth" type="number" min="1" max="99" value={nth} onChange={(event) => setNth(Math.max(1, Number(event.target.value)))}/></label><label>Offset <input aria-label="Every Nth offset" type="number" min="0" max="98" value={offset} onChange={(event) => setOffset(Math.max(0, Number(event.target.value)))}/></label><button onClick={() => macro({ type: "every_nth", n: nth, offset })}>Apply</button><button onClick={() => void server.storeGroup(String(stored.length + 1), `Group ${stored.length + 1}`)}>Store Group</button></div>}</header>
    <div className="card-pool">{cards.map((group, index) => <GroupCard key={index + 1} group={group} index={index} patched={patched} capabilities={capabilities} selected={server.selectedGroupId === group?.id} beginHold={() => { if (group) hold.current = window.setTimeout(() => setContextGroup(group.id), 600); }} cancelHold={cancelHold} openContext={() => group && setContextGroup(group.id)} select={() => group ? void server.selectGroup(group.id) : void server.storeGroup(String(index + 1), `Group ${index + 1}`)} setMaster={(value) => group && void server.setGroupMaster(group.id, value)}/>)}</div>
    {contextual && <div className="group-context-menu"><h3>{contextual.body.name ?? `Group ${contextual.id}`}</h3><small className="group-order">Ordered members: {contextual.body.fixtures.length ? contextual.body.fixtures.map((fixture, index) => `${index + 1}. ${fixtureNames.get(fixture) ?? fixture}`).join(" · ") : "empty"}</small><button onClick={() => { void server.selectGroup(contextual.id); setContextGroup(null); }}>Select live group</button><button onClick={() => { void server.selectGroup(contextual.id, true); setContextGroup(null); }}>Select frozen group</button>{contextual.body.frozen_from && <button onClick={() => { void server.refreshFrozenGroup(contextual.id); setContextGroup(null); }}>Refresh frozen snapshot</button>}{contextual.body.derived_from ? <button onClick={() => { void server.detachDerivedGroup(contextual.id); setContextGroup(null); }}>Detach derived group</button> : <button onClick={() => { const count = Object.keys(contextual.body.programming ?? {}).length; if (!count || window.confirm(`Replace membership and apply ${count} stored attributes to the new members?`)) void server.storeGroup(contextual.id, contextual.body.name ?? `Group ${contextual.id}`); setContextGroup(null); }}>Replace membership with selection</button>}<button onClick={() => { void server.undoGroup(contextual.id); setContextGroup(null); }}>Undo membership/programming change</button><button onClick={() => setContextGroup(null)}>Cancel</button></div>}
  </div>;
}

function GroupCard({ group, index, patched, capabilities, selected, beginHold, cancelHold, openContext, select, setMaster }: { group: VersionedObject<StoredGroup> | null; index: number; patched: Set<string>; capabilities: Map<string, Set<string>>; selected: boolean; beginHold: () => void; cancelHold: () => void; openContext: () => void; select: () => void; setMaster: (value: number) => void }) {
  const missing = group?.body.fixtures.filter((fixture) => !patched.has(fixture)).length ?? 0;
  const attributes = Object.keys(group?.body.programming ?? {});
  const unsupported = group?.body.fixtures.reduce((count, fixture) => count + attributes.filter((attribute) => capabilities.has(fixture) && !capabilities.get(fixture)!.has(attribute)).length, 0) ?? 0;
  return <article className={`group-card-shell ${group?.body.derived_from ? "derived" : ""} ${group?.body.frozen_from ? "frozen" : ""}`}><button className={`group-card ${selected ? "selected" : !group ? "empty" : ""}`} onPointerDown={beginHold} onPointerUp={cancelHold} onPointerCancel={cancelHold} onContextMenu={(event) => { event.preventDefault(); openContext(); }} onClick={select}><span className="number">{index + 1}</span>{group ? <><b>{group.body.name ?? `Group ${index + 1}`}</b><small>{group.body.fixtures.length ? `${group.body.fixtures.length} fixtures · ordered` : "⚠ Group is empty"}</small>{missing > 0 && <em>⚠ {missing} missing/unpatched</em>}{attributes.length > 0 && <em>{attributes.length} portable attributes</em>}{unsupported > 0 && <em>⚠ {unsupported} unsupported values</em>}{group.body.derived_from && <em>Derived · {group.body.derived_from.rule.type}</em>}{group.body.frozen_from && <em>Frozen · rev {group.body.frozen_from.source_revision}</em>}</> : <><b>Empty</b><small>Tap to create empty group</small></>}</button>{group && <label className="group-master">Fader {group.body.playback_fader ?? "—"} · Master <strong>{Math.round((group.body.master ?? 1) * 100)}%</strong><input aria-label={`${group.body.name ?? `Group ${index + 1}`} master`} type="range" min="0" max="100" value={(group.body.master ?? 1) * 100} onChange={(event) => setMaster(Number(event.target.value) / 100)}/></label>}</article>;
}
