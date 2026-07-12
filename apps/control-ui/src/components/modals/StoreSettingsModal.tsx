import { useState } from "react";
import { useApp } from "../../state/AppContext";

export function StoreSettingsModal() {
  const { state, dispatch } = useApp();
  const [mode, setMode] = useState<"merge" | "overwrite">(() => localStorage.getItem("light.store-mode") === "overwrite" ? "overwrite" : "merge");
  const [mergeActiveCue, setMergeActiveCue] = useState(() => localStorage.getItem("light.store-merge-active-cue") === "true");
  if (!state.storeSettingsOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: false });
  const chooseMode = (value: "merge" | "overwrite") => { setMode(value); localStorage.setItem("light.store-mode", value); };
  return <div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="modal-card store-settings-modal"><button className="modal-close" onClick={close}>×</button><h2>Record Settings</h2><p>Defaults for the armed Record workflow.</p><div className="segmented-control"><button className={mode === "merge" ? "active" : ""} onClick={() => chooseMode("merge")}>Merge</button><button className={mode === "overwrite" ? "active" : ""} onClick={() => chooseMode("overwrite")}>Overwrite</button></div><label className="store-setting-toggle"><input type="checkbox" checked={mergeActiveCue} onChange={(event) => { setMergeActiveCue(event.target.checked); localStorage.setItem("light.store-merge-active-cue", String(event.target.checked)); }}/> Merge current values into the active cue when storing to its playback</label><div className="modal-actions"><button onClick={close}>Done</button></div></section></div>;
}
