import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { HorizontalTouchFader } from "../control/HorizontalTouchFader";

export function SystemControlsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [master, setMaster] = useState(100);
  const [blackout, setBlackout] = useState(false);
  const [lampResult, setLampResult] = useState("");
  useEffect(() => { if (!state.systemControlsOpen) return; void server.readVisualization().then((snapshot) => { setMaster(Math.round(snapshot.grand_master * 100)); setBlackout(snapshot.blackout); dispatch({ type: "SET_BLACKOUT", value: snapshot.blackout }); }); }, [state.systemControlsOpen, server.readVisualization, dispatch]);
  const lampCommands = useMemo(() => {
    const commands: Array<{ fixtureId: string; attribute: string; value: number }> = [];
    for (const fixture of server.patch?.fixtures ?? []) {
      if (!server.selectedFixtures.includes(fixture.fixture_id) && !fixture.logical_heads.some((head) => server.selectedFixtures.includes(head.fixture_id))) continue;
      for (const head of fixture.definition.heads ?? []) for (const parameter of head.parameters) {
        const capability = parameter.capabilities?.find((item) => item.name.toLowerCase().includes("lamp on"));
        if (capability) { commands.push({ fixtureId: fixture.fixture_id, attribute: parameter.attribute, value: ((capability.dmx_from + capability.dmx_to) / 2) / 255 }); break; }
      }
    }
    return commands;
  }, [server.patch, server.selectedFixtures]);
  const allLampsOn = async () => {
    const supported = new Set(lampCommands.map((item) => item.fixtureId));
    await Promise.all(lampCommands.map((item) => server.setProgrammer(item.fixtureId, item.attribute, item.value)));
    const skipped = Math.max(0, server.selectedFixtures.length - supported.size);
    setLampResult(`${supported.size} lamp${supported.size === 1 ? "" : "s"} triggered${skipped ? ` · ${skipped} unsupported skipped` : ""}`);
  };
  if (!state.systemControlsOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: false });
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><section className="modal-card system-controls-card">
    <header className="system-controls-header"><h2>Output & Timecode Controls</h2><button className="modal-close" onClick={close}>×</button></header>
    <section className="master-controls"><HorizontalTouchFader label="Grand master" value={master} onChange={(value) => { setMaster(value); void server.setMaster(value / 100, undefined); }}/><button className={blackout ? "danger active" : "danger"} onClick={() => { const next = !blackout; setBlackout(next); dispatch({ type: "SET_BLACKOUT", value: next }); void server.setMaster(undefined, next); }}>{blackout ? "RELEASE BLACKOUT" : "BLACKOUT"}</button><button className="lamp-on-all" disabled={!server.selectedFixtures.length} onClick={() => void allLampsOn()}>All Lamps On</button></section>{lampResult && <p className="lamp-command-result">{lampResult}</p>}
    <h3>Active programmers</h3><div className="programmer-list">{server.bootstrap?.active_programmers.map((programmer) => <article key={programmer.user_id}><span><b>{programmer.user_id === server.session?.user.id ? `${server.session.user.name} · Current user` : `User ${programmer.user_id.slice(0, 8)}`}</b><small>{programmer.selected.length} fixtures · {programmer.values.length} values · {programmer.connected ? "Connected" : "Disconnected"}</small></span><button className="danger" onClick={() => void server.clearProgrammer(programmer.session_id)}>Clear</button></article>)}{!server.bootstrap?.active_programmers.length && <p className="empty-window-message">No active programmers.</p>}</div>
  </section></div>;
}
