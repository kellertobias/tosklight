import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { HorizontalTouchFader } from "../control/HorizontalTouchFader";
import { Button, ModalPortal } from "../common";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";

function countProgrammerValues(programmer: { values: unknown[]; group_values?: Record<string, Record<string, unknown>> }) {
  return programmer.values.length + Object.values(programmer.group_values ?? {}).reduce((total, values) => total + Object.keys(values).length, 0);
}

export function SystemControlsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [master, setMaster] = useState(100);
  const [blackout, setBlackout] = useState(false);
  const [lampResult, setLampResult] = useState("");
  const [stoppingAll, setStoppingAll] = useState(false);
  useEffect(() => { if (!state.systemControlsOpen) return; void server.readVisualization().then((snapshot) => { setMaster(Math.round(snapshot.grand_master * 100)); setBlackout(snapshot.blackout); dispatch({ type: "SET_BLACKOUT", value: snapshot.blackout }); }); }, [state.systemControlsOpen, server.readVisualization, dispatch]);
  const lampActions = useMemo(() => compatibleSpecialDialogActions(
    server.patch?.fixtures ?? [],
    "lamp_on",
    server.selectedFixtures,
  ), [server.patch, server.selectedFixtures]);
  const allLampsOn = async (phase: "click" | "press" | "release") => {
    const actions = lampActions.filter((action) =>
      phase === "click" ? action.kind !== "momentary" : action.kind === "momentary",
    );
    await Promise.all(actions.map((action) =>
      server.controlFixtureAction(action.fixtureId, action.actionId, phase !== "release"),
    ));
    const supported = new Set(lampActions.map((item) => item.fixtureId));
    const skipped = Math.max(0, server.selectedFixtures.length - supported.size);
    setLampResult(`${supported.size} discharge lamp${supported.size === 1 ? "" : "s"} triggered${skipped ? ` · ${skipped} without Lamp On skipped` : ""}`);
  };
  if (!state.systemControlsOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: false });
  const runningPlaybacks = server.playbacks?.active ?? [];
  const pagePlaybacks = runningPlaybacks.filter((playback) => playback.playback_number != null);
  const virtualPlaybacks = runningPlaybacks.filter((playback) => playback.playback_number == null);
  const activeProgrammers = server.bootstrap?.active_programmers ?? [];
  const runningDynamics = runningPlaybacks.flatMap((playback) => {
    const cueList = server.playbacks?.cue_lists.find((candidate) => candidate.id === playback.cue_list_id);
    const cue = cueList?.cues[playback.cue_index];
    return (cue?.phasers ?? []).map((_, index) => ({ playback, cueList, cue, index }));
  });
  const playbackRow = (playback: (typeof runningPlaybacks)[number], source: "Playback" | "Virtual playback") => {
    const cueList = server.playbacks?.cue_lists.find((candidate) => candidate.id === playback.cue_list_id);
    const cue = cueList?.cues[playback.cue_index];
    const definition = playback.playback_number == null ? null : server.playbacks?.pool.find((candidate) => candidate.number === playback.playback_number);
    const label = definition?.name || cueList?.name || `Cuelist ${playback.cue_list_id.slice(0, 8)}`;
    return <article key={playback.cue_list_id}>
      <span><b>{label}</b><small>{playback.playback_number == null ? source : `Playback ${playback.playback_number}`} · Cue {cue?.number ?? playback.cue_index + 1} · {Math.round(playback.master * 100)}% · {playback.paused ? "Paused" : "Running"}</small></span>
      <Button className="danger" aria-label={`Stop ${source} ${label}`} onClick={() => void server.playbackAction(playback.cue_list_id, "release")}>Stop</Button>
    </article>;
  };
  const stopEverything = async () => {
    setStoppingAll(true);
    try {
      await Promise.all([
        ...runningPlaybacks.map((playback) => server.playbackAction(playback.cue_list_id, "release")),
        ...activeProgrammers.map((programmer) => server.clearProgrammer(programmer.session_id)),
        server.preloadAction("release"),
      ]);
      dispatch({ type: "RELEASE_PRELOAD" });
    } finally {
      setStoppingAll(false);
    }
  };
  return <ModalPortal><div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><section className="modal-card system-controls-card" role="dialog" aria-modal="true" aria-label="Running and output">
    <header className="system-controls-header"><div><h2>Running & Output</h2><small>Shift + Clear / Shift + Delete</small></div><Button className="modal-close" onClick={close}>×</Button></header>
    <div className="running-summary"><span><b>{pagePlaybacks.length + virtualPlaybacks.length + activeProgrammers.length + runningDynamics.length}</b> active items</span><Button className="danger" disabled={stoppingAll || (!runningPlaybacks.length && !activeProgrammers.length)} onClick={() => void stopEverything()}>{stoppingAll ? "Stopping…" : "Stop everything"}</Button></div>
    <div className="running-sections">
      <section><h3>Virtual playbacks <small>{virtualPlaybacks.length}</small></h3><div className="programmer-list">{virtualPlaybacks.map((playback) => playbackRow(playback, "Virtual playback"))}{!virtualPlaybacks.length && <p className="empty-window-message">No virtual playbacks are running.</p>}</div></section>
      <section><h3>Playbacks <small>{pagePlaybacks.length}</small></h3><div className="programmer-list">{pagePlaybacks.map((playback) => playbackRow(playback, "Playback"))}{!pagePlaybacks.length && <p className="empty-window-message">No playbacks are running.</p>}</div></section>
      <section><h3>Active programmers <small>{activeProgrammers.length}</small></h3><div className="programmer-list">{activeProgrammers.map((programmer) => <article key={programmer.session_id}><span><b>{programmer.user_id === server.session?.user.id ? `${server.session.user.name} · Current user` : `User ${programmer.user_id.slice(0, 8)}`}</b><small>{programmer.selected.length} fixtures · {countProgrammerValues(programmer)} values · {programmer.connected ? "Connected" : "Disconnected"}</small></span><Button className="danger" aria-label={`Clear programmer ${programmer.user_id}`} onClick={() => void server.clearProgrammer(programmer.session_id)}>Clear</Button></article>)}{!activeProgrammers.length && <p className="empty-window-message">No active programmers.</p>}</div></section>
      <section><h3>Dynamics <small>{runningDynamics.length}</small></h3><div className="programmer-list">{runningDynamics.map(({ playback, cueList, cue, index }) => <article key={`${playback.cue_list_id}-${index}`}><span><b>{cueList?.name ?? "Cuelist"} · Dynamic {index + 1}</b><small>Cue {cue?.number ?? playback.cue_index + 1} · Stop releases its source playback</small></span><Button className="danger" title="Stops this Dynamic by releasing its source playback" aria-label={`Stop Dynamic ${index + 1} from ${cueList?.name ?? "Cuelist"}`} onClick={() => void server.playbackAction(playback.cue_list_id, "release")}>Stop</Button></article>)}{!runningDynamics.length && <p className="empty-window-message">No dynamics are running.</p>}</div></section>
    </div>
    <h3>Output controls</h3><section className="master-controls"><HorizontalTouchFader label="Grand master" value={master} onChange={(value) => { setMaster(value); void server.setMaster(value / 100, undefined); }}/><Button className={blackout ? "danger active" : "danger"} onClick={() => { const next = !blackout; setBlackout(next); dispatch({ type: "SET_BLACKOUT", value: next }); void server.setMaster(undefined, next); }}>{blackout ? "RELEASE BLACKOUT" : "BLACKOUT"}</Button><Button className="lamp-on-all" disabled={!server.selectedFixtures.length} onClick={() => void allLampsOn("click")} onPointerDown={() => void allLampsOn("press")} onPointerUp={() => void allLampsOn("release")} onPointerCancel={() => void allLampsOn("release")} onKeyDown={(event: KeyboardEvent) => { if (!event.repeat && (event.key === "Enter" || event.key === " ")) void allLampsOn("press"); }} onKeyUp={(event: KeyboardEvent) => { if (event.key === "Enter" || event.key === " ") void allLampsOn("release"); }}>All Lamps On</Button></section>{lampResult && <p className="lamp-command-result">{lampResult}</p>}
  </section></div></ModalPortal>;
}
