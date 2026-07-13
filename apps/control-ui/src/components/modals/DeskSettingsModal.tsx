import { useEffect, useState } from "react";
import { useApp } from "../../state/AppContext";
import { Button, Input } from "../common";

const icons = ["⊞", "⌂", "★", "◉", "▶", "▣", "⚙", "◇"];

export function DeskSettingsModal() {
  const { state, dispatch } = useApp();
  const desk = state.desks.find((item) => item.id === state.deskSettingsId);
  const [name, setName] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  useEffect(() => { setName(desk?.name ?? ""); setConfirmDelete(false); }, [desk?.id]);
  if (!state.deskSettingsOpen || !desk) return null;
  const close = () => dispatch({ type: "OPEN_DESK_SETTINGS", id: null });
  return <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && close()}><section className="nested-modal desk-settings-modal" role="dialog" aria-modal="true" aria-label="Desk settings"><Button className="modal-close" onClick={close}>×</Button><h3>Desk View</h3><label>Name<Input value={name} onChange={(event) => setName(event.target.value)} onBlur={() => name.trim() && dispatch({ type: "UPDATE_DESK", id: desk.id, name: name.trim() })}/></label><span>Icon</span><div className="desk-icon-picker">{icons.map((icon) => <Button className={(desk.icon ?? "⊞") === icon ? "active" : ""} key={icon} onClick={() => dispatch({ type: "UPDATE_DESK", id: desk.id, icon })}>{icon}</Button>)}</div>{confirmDelete ? <div className="delete-confirm"><b>Delete “{desk.name}”?</b><Button onClick={() => setConfirmDelete(false)}>Cancel</Button><Button className="danger" onClick={() => dispatch({ type: "DELETE_DESK", id: desk.id })}>Confirm Delete</Button></div> : <Button className="danger large-danger" disabled={state.desks.length <= 1} onClick={() => setConfirmDelete(true)}>Delete Desk View</Button>}</section></div>;
}
