import { useMemo, useState } from "react";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { Button, FormLayout, NumberField, SelectField, TextField } from "../common";

export function PreloadStoreModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [target, setTarget] = useState<"preset" | "cue">("preset"), [targetId, setTargetId] = useState("1");
  const [cueNumber, setCueNumber] = useState(1), [name, setName] = useState("");
  const [mode, setMode] = useState<"merge" | "overwrite" | "add_missing_fixtures">("merge");
  const targetObject = useMemo(() => target === "preset" ? server.presets.find((object) => object.id === targetId) : server.cueObjects.find((object) => object.id === targetId), [target, targetId, server.presets, server.cueObjects]);
  if (!state.preloadStoreOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "preloadStoreOpen", value: false });
  const submit = async () => { if (await server.storePreload({ target, target_id: targetId, cue_number: target === "cue" ? cueNumber : undefined, name: name || undefined, mode: target === "preset" ? mode : undefined }, targetObject?.revision ?? 0)) close(); };
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><section className="modal-card preload-store-card"><Button className="modal-close" onClick={close}>×</Button><h2>Record Pending Preload</h2><p>The active preload scene remains live. Only the pending scene will be stored.</p><div className="segmented-control"><Button className={target === "preset" ? "active" : ""} onClick={() => setTarget("preset")}>Preset</Button><Button className={target === "cue" ? "active" : ""} onClick={() => { setTarget("cue"); setTargetId(server.cueObjects[0]?.id ?? ""); }}>Cue</Button></div><FormLayout className="preload-target-form" labelPlacement="side">{target === "preset" ? <TextField label="Preset slot" value={targetId} onChange={(event) => setTargetId(event.target.value)}/> : <SelectField label="Cue list" value={targetId} onChange={setTargetId} options={server.cueObjects.map((cue) => ({ value: cue.id, label: String(cue.body.name ?? cue.id) }))}/>} {target === "cue" && <NumberField label="Cue number" allowDecimal step="0.1" value={cueNumber} onChange={(event) => setCueNumber(Number(event.target.value))}/>} {target === "preset" && <SelectField label="Record mode" value={mode} onChange={setMode} options={[{ value: "merge", label: "Merge" }, { value: "overwrite", label: "Overwrite" }, { value: "add_missing_fixtures", label: "Add missing fixtures" }]}/>}<TextField label="Name" clearable value={name} onChange={(event) => setName(event.target.value)} placeholder="Optional name"/></FormLayout><div className="modal-actions"><Button onClick={close}>Cancel</Button><Button disabled={!targetId} onClick={() => void submit()}>Record to {target === "preset" ? `Preset ${targetId}` : `Cue ${cueNumber}`}</Button></div>{targetObject && <small>Existing target revision {targetObject.revision}; normal conflict protection applies.</small>}{server.error && <p className="modal-error">{server.error}</p>}</section></div>;
}
