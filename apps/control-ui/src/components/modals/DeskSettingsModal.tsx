import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { Button, FormLayout, IconPickerField, TextField } from "../common";

export function DeskSettingsModal() {
  const { state, dispatch } = useApp();
  const desk = state.desks.find((item) => item.id === state.deskSettingsId);
  const [name, setName] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  useEffect(() => { setName(desk?.name ?? ""); setConfirmDelete(false); }, [desk?.id]);
  if (!state.deskSettingsOpen || !desk) return null;
  const close = () => dispatch({ type: "OPEN_DESK_SETTINGS", id: null });
  const clone = () => {
    dispatch({ type: "START_SAVE_DESK" });
    dispatch({ type: "NEW_DESK" });
    close();
  };
  return <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="nested-modal desk-settings-modal" role="dialog" aria-modal="true" aria-label="Desktop settings"><Button className="modal-close" onClick={close}>×</Button><h3>Desktop</h3><FormLayout labelPlacement="side"><TextField label="Name" clearable value={name} onChange={(event) => setName(event.target.value)} onBlur={() => name.trim() && dispatch({ type: "UPDATE_DESK", id: desk.id, name: name.trim() })}/><IconPickerField label="Icon" value={desk.icon ?? "⊞"} onChange={(icon) => dispatch({ type: "UPDATE_DESK", id: desk.id, icon })}/></FormLayout><Button onClick={clone}>Clone current desktop</Button>{confirmDelete ? <div className="delete-confirm"><b>Delete desktop “{desk.name}”?</b><Button onClick={() => setConfirmDelete(false)}>Cancel</Button><Button className="danger" onClick={() => dispatch({ type: "DELETE_DESK", id: desk.id })}>Confirm delete</Button></div> : <Button className="danger large-danger" disabled={state.desks.length <= 1} onClick={() => setConfirmDelete(true)}>Delete desktop</Button>}</section></div>;
}
