import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import {
  loadRecordSettings,
  RecordDefaultsFields,
  saveRecordSettings,
} from "../setup/ProgrammerDefaults";

export function StoreSettingsModal() {
  const { state, dispatch } = useApp();
  const [settings, setSettings] = useState(loadRecordSettings);
  useEffect(() => {
    if (state.storeSettingsOpen) setSettings(loadRecordSettings());
  }, [state.storeSettingsOpen]);
  if (!state.storeSettingsOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "storeSettingsOpen", value: false });
  const change = (next: typeof settings) => {
    setSettings(next);
    saveRecordSettings(next);
  };
  return <div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="modal-card store-settings-modal"><Button className="modal-close" onClick={close}>×</Button><h2>Record Settings</h2><p>Defaults for the armed Record workflow.</p><RecordDefaultsFields settings={settings} onChange={change}/><div className="modal-actions"><Button onClick={close}>Done</Button></div></section></div>;
}
