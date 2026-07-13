import { groups } from "../../data/mockData";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";

export function GroupStrip() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const stored = server.bootstrap ? server.groups : groups.slice(0, 10).map((group) => ({ id: String(group.id), body: { name: group.name, fixtures: Array.from({ length: group.fixtures }, (_, index) => String(index)) } }));
  const visible = Array.from({ length: 10 }, (_, index) => stored.find((group) => group.id === String(index + 1)) ?? null);
  return <section className="group-strip"><header><b>Group shortcuts</b><small>slots 1–10</small></header><div>{visible.map((group, index) => <Button onClick={() => { if (group && !state.storeArmed) return void server.applyGroup(group.id); if (group) { const mode = window.confirm("Merge the current selection into this group? Choose Cancel to overwrite it instead.") ? "merge" : "overwrite"; void server.storeGroup(group.id, group.body.name ?? `Group ${group.id}`, mode); dispatch({ type: "SET_STORE_ARMED", value: false }); return; } if (!state.storeArmed) return; void server.storeGroup(String(index + 1), `Group ${index + 1}`); dispatch({ type: "SET_STORE_ARMED", value: false }); }} className={`group-card pool-cell ${server.selectedGroupId === group?.id ? "selected" : ""} ${group ? "" : "empty"} ${state.storeArmed && !group ? "store-target" : ""}`} key={group?.id ?? `empty-${index + 1}`}><span className="number">{index + 1}</span><b>{group?.body.name ?? "Empty"}</b><small>{group ? group.body.fixtures.length ? `${group.body.fixtures.length} fixtures` : "Group is empty" : state.storeArmed ? "Tap to record" : "Press Rec first"}</small></Button>)}</div></section>;
}
