import { groups } from "../../data/mockData";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function GroupStrip() {
  const server = useServer();
  const { dispatch } = useApp();
  const visible = server.bootstrap ? server.groups.slice(0, 10) : groups.slice(0, 10).map((group) => ({ id: String(group.id), body: { name: group.name, fixtures: Array.from({ length: group.fixtures }, (_, index) => String(index)) } }));
  return <section className="group-strip"><header><b>Group shortcuts</b><small>first 10 configured groups</small><button aria-label="Open group pool" onClick={() => dispatch({ type: "OPEN_BUILTIN", kind: "groups" })}>⚙</button></header><div>{visible.map((group) => <button onClick={() => void server.applyGroup(group.id)} className={`group-card ${server.selectedGroupId === group.id ? "selected" : ""}`} key={group.id}><b>{group.body.name ?? `Group ${group.id}`}</b><small>{group.body.fixtures.length ? `${group.body.fixtures.length} fixtures` : "⚠ Group is empty"}</small></button>)}</div></section>;
}
