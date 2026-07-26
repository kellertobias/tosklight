import { forwardRef, useCallback, useImperativeHandle, useState } from "react";
import { Button } from "../common";
import { ModalCaretValue, ModalNumberInput } from "../input/ModalInputControls";
import { ModalLayer } from "../modals/ModalStack";
import { submitNumericExpression } from "../input/numericExpression";

export type HardwareEncoderTarget = {
  label: string;
  value: string;
  role?: string;
};

export interface HardwareEncoderDisplayHandle {
  activate(): void;
}

export interface HardwareEncoderDisplayProps {
  slot: number;
  target?: HardwareEncoderTarget;
  secondary?: HardwareEncoderTarget;
  editValue?: number;
  onEdit?: (value: number) => void;
  onEditRange?: (points: number[]) => void;
  canRelease?: boolean;
  onRelease?: () => void;
}

export const HardwareEncoderDisplayView = forwardRef<HardwareEncoderDisplayHandle, HardwareEncoderDisplayProps>(function HardwareEncoderDisplayView({
  slot,
  target,
  secondary,
  editValue,
  onEdit,
  onEditRange,
  canRelease = false,
  onRelease,
}, ref) {
  const [editing, setEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [caret, setCaret] = useState(0);
  const openEditor = useCallback(() => {
    const next = String(Number((editValue ?? 0).toFixed(1)));
    setInputValue(next);
    setCaret(next.length);
    setEditing(true);
  }, [editValue]);
  useImperativeHandle(ref, () => ({ activate: openEditor }), [openEditor]);
  const submit = () => {
    if (submitNumericExpression(inputValue, onEdit, onEditRange)) setEditing(false);
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
    {editing && <ModalLayer ariaLabel={`Encoder ${slot} value`}
      dialogClassName="direct-value-modal hardware-encoder-modal"
      onClose={() => setEditing(false)}>
      <Button className="modal-close" aria-label="Close encoder value" onClick={() => setEditing(false)}>×</Button>
      <h3>{target.label}</h3><ModalCaretValue value={inputValue} caret={caret} />
      <ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submit} onEscape={() => setEditing(false)} onCaretChange={setCaret} replaceOnFirstInput allowThrough={Boolean(onEditRange)} />
      {canRelease && onRelease && <footer className="modal-actions"><Button variant="danger" aria-label={`Release ${target.label}`} onClick={() => { onRelease(); setEditing(false); }}>Release</Button></footer>}
    </ModalLayer>}
  </>;
});
