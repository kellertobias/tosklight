import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { Button, ModalPortal } from "../common";
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
  return <ModalPortal><div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="modal-card store-settings-modal workflow-theme record-workflow" role="dialog" aria-modal="true" aria-label="Record Settings"><Button className="modal-close" aria-label="Close Record Settings" onClick={close}>×</Button><h2><span className="workflow-badge">RECORD</span> Record Settings</h2><p>Defaults for the armed Record workflow.</p><RecordDefaultsFields settings={settings} onChange={change}/><div className="modal-actions"><Button className="workflow-primary" onClick={close}>Done</Button></div></section></div></ModalPortal>;
}
