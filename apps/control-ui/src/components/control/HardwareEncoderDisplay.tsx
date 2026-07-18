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
  editValue,
  onEdit,
  onEditRange,
  onRelease,
}: {
  slot: number;
  target?: HardwareEncoderTarget;
  secondary?: HardwareEncoderTarget;
  editValue?: number;
  onEdit?: (value: number) => void;
  onEditRange?: (points: number[]) => void;
  onRelease?: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const openEditor = () => {
    setInputValue(String(Number((editValue ?? 0).toFixed(1))));
    setEditing(true);
  };
  const submit = () => {
    const points = inputValue.split(/\s+THRU\s+/i).map((part) => Number(part.trim()));
    if (points.length > 1) {
      if (!onEditRange || points.some((value) => !Number.isFinite(value))) return;
      onEditRange(points);
      setEditing(false);
      return;
    }
    const value = Number(inputValue);
    if (Number.isFinite(value)) onEdit?.(value);
    setEditing(false);
  };
  if (!target) return <section className="hardware-encoder-display unassigned" aria-label={`Encoder ${slot} unassigned`}>
    <header><b>Unassigned</b><small>Enc {slot}</small></header>
  </section>;
  const content = <>
      <header><b title={target.label}>{target.label}</b><small>Enc {slot}</small></header>
      <div className="hardware-encoder-target"><strong>{target.value}</strong>{target.role && <span>{target.role}</span>}</div>
      {secondary && <div className="hardware-encoder-target secondary"><b title={secondary.label}>{secondary.label}</b><strong>{secondary.value}</strong>{secondary.role && <span>{secondary.role}</span>}</div>}
  </>;
  const displayClassName = `hardware-encoder-display ${secondary ? "dual-target" : "single-target"}`;
  return <>
    {onEdit
      ? <Button className={displayClassName} aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`} onClick={openEditor}>{content}</Button>
      : <section className={displayClassName} aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}>{content}</section>}
    {editing && createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setEditing(false)}><section className="nested-modal direct-value-modal hardware-encoder-modal" role="dialog" aria-modal="true" aria-label={`Encoder ${slot} value`}>
      <Button className="modal-close" aria-label="Close encoder value" onClick={() => setEditing(false)}>×</Button>
      <h3>{target.label}</h3><strong>{inputValue || "0"}</strong>
      <ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submit} onEscape={() => setEditing(false)} replaceOnFirstInput allowThrough={Boolean(onEditRange)} />
      {onRelease && <footer className="modal-actions"><Button variant="danger" aria-label={`Release ${target.label}`} onClick={() => { onRelease(); setEditing(false); }}>Release</Button></footer>}
    </section></div>, document.body)}
  </>;
}
