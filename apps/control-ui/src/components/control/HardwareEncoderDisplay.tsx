import { useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";

export type HardwareEncoderTarget = {
  label: string;
  value: string;
  role?: string;
};

export function HardwareEncoderDisplay({
  slot,
  target,
  secondary,
  state,
  editValue,
  onEdit,
  onRelease,
}: {
  slot: number;
  target?: HardwareEncoderTarget;
  secondary?: HardwareEncoderTarget;
  state?: string;
  editValue?: number;
  onEdit?: (value: number) => void;
  onRelease?: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const openEditor = () => {
    setInputValue(String(Number((editValue ?? 0).toFixed(1))));
    setEditing(true);
  };
  const submit = () => {
    const value = Number(inputValue);
    if (Number.isFinite(value)) onEdit?.(value);
    setEditing(false);
  };
  if (!target) return <section className="hardware-encoder-display unassigned" aria-label={`Encoder ${slot} unassigned`}>
    <small>Enc {slot}</small><b>Unassigned</b><strong>—</strong><span>Not mapped</span>
  </section>;
  return <>
    <section className="hardware-encoder-display" aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}>
      <small>Enc {slot}</small>
      <div className="hardware-encoder-target"><b title={target.label}>{target.label}</b><strong>{target.value}</strong><span>{target.role ?? "Turn"}</span></div>
      {secondary && <div className="hardware-encoder-target secondary"><b title={secondary.label}>{secondary.label}</b><strong>{secondary.value}</strong><span>{secondary.role ?? "Press-turn"}</span></div>}
      {state && <i>{state}</i>}
      {(onEdit || onRelease) && <footer>
        {onEdit && <Button aria-label={`Set value for ${target.label}`} onClick={openEditor}>Set value</Button>}
        {onRelease && <Button aria-label={`Release ${target.label}`} onClick={onRelease}>Release</Button>}
      </footer>}
    </section>
    {editing && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setEditing(false)}><section className="nested-modal direct-value-modal hardware-encoder-modal" role="dialog" aria-modal="true" aria-label={`Encoder ${slot} value`}>
      <Button className="modal-close" aria-label="Close encoder value" onClick={() => setEditing(false)}>×</Button>
      <h3>{target.label}</h3><strong>{inputValue || "0"}</strong>
      <ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submit} onEscape={() => setEditing(false)} replaceOnFirstInput />
    </section></div>, document.body)}
  </>;
}
