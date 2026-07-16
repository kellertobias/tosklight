import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button, Input, type ButtonProps } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export interface VerticalTouchFaderAction extends Omit<ButtonProps, "children"> { id: string; label: ReactNode }
export interface VerticalTouchFaderProps { label: string; value: number; maximum?: number; display?: string; disabled?: boolean; accentColor?: string; mode?: string; directInput?: boolean; directInputOffset?: number; actions?: VerticalTouchFaderAction[]; onChange?: (value: number) => void }

interface SetValueDialogProps {
  label: string;
  value: string;
  maximum: number;
  offset: number;
  onChange: (value: string) => void;
  onFaderChange: (value: number) => void;
  onSubmit: () => void;
  onClose: () => void;
}

function SetValueDialog({ label, value, maximum, offset, onChange, onFaderChange, onSubmit, onClose }: SetValueDialogProps) {
  const entered = Number(value);
  const faderValue = Number.isFinite(entered) ? Math.max(0, Math.min(maximum, entered + offset)) : offset;
  return createPortal(<div className="stacked-modal-layer" onClick={(event)=>event.stopPropagation()} onPointerDown={(event)=>event.target===event.currentTarget&&onClose()}><section className="nested-modal direct-value-modal" role="dialog" aria-modal="true" aria-label={`${label} value`}><Button className="modal-close" aria-label="Close attribute value" onClick={onClose}>×</Button><h3>{label}</h3><strong>{value || "0"}</strong><div className="direct-value-modal-body"><VerticalTouchFader label={label} value={faderValue} maximum={maximum} display={value || "0"} onChange={onFaderChange}/><ModalNumberInput value={value} onChange={onChange} onEnter={onSubmit} onEscape={onClose} replaceOnFirstInput/></div></section></div>,document.body);
}

export function VerticalTouchFader({ label, value, maximum = 100, display, disabled = false, accentColor, mode, directInput = false, directInputOffset = 0, actions = [], onChange }: VerticalTouchFaderProps) {
  const server = useServer(); const { state } = useApp(); const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const [localValue, setLocalValue] = useState(value);
  const [inputOpen, setInputOpen] = useState(false); const [inputValue, setInputValue] = useState("");
  const interacting = useRef(false);
  const frame = useRef<number | null>(null);
  const queued = useRef(value);
  useEffect(() => {
    if (!interacting.current) setLocalValue(value);
  }, [value]);
  useEffect(() => () => {
    if (frame.current !== null) window.cancelAnimationFrame(frame.current);
  }, []);
  const emit = (next: number) => {
    queued.current = next;
    setLocalValue(next);
    if (frame.current !== null) return;
    frame.current = window.requestAnimationFrame(() => {
      frame.current = null;
      onChange?.(queued.current);
    });
  };
  const finish = () => {
    interacting.current = false;
    if (frame.current !== null) {
      window.cancelAnimationFrame(frame.current);
      frame.current = null;
    }
    onChange?.(queued.current);
  };
  const fraction = Math.max(0, Math.min(1, maximum ? localValue / maximum : 0));
  const openInput = () => { if (!disabled && directInput) { setInputValue(String(Number((localValue - directInputOffset).toFixed(1)))); setInputOpen(true); } };
  const submitInput = () => { const entered = Number(inputValue); const next = Math.max(0, Math.min(maximum, entered + directInputOffset)); if (Number.isFinite(entered)) { setLocalValue(next); onChange?.(next); } setInputOpen(false); };
  const fader = <label onClick={() => hardware && openInput()} className={`vertical-touch-fader ${disabled ? "disabled" : ""} ${directInput ? "direct-input-fader" : ""}`} style={{ "--fader-level": fraction, "--fader-color": accentColor ?? "#176777", "--fader-color-dark": accentColor ? `color-mix(in srgb, ${accentColor} 42%, #081014)` : "#103039" } as CSSProperties}>
    <span>{label}{mode && <small>{mode}</small>}</span><strong>{display === undefined ? `${Math.round(localValue)}%` : display.replace(/^[\d.]+/, String(Math.round(localValue)))}</strong>
    <Input aria-label={label} disabled={disabled || (hardware && directInput)} type="range" min="0" max={maximum} step="0.1" value={localValue} onPointerDown={() => { interacting.current = true; }} onPointerUp={finish} onPointerCancel={finish} onBlur={() => { if (interacting.current) finish(); }} onInput={(event) => emit(Number(event.currentTarget.value))}/>
  </label>;
  const visibleActions = [...(directInput && !hardware ? [{ id: "set-value", label: "Set value", onClick: openInput, className: "set-value-button" } satisfies VerticalTouchFaderAction] : []), ...actions].slice(0, 3);
  return <div className={`vertical-touch-fader-stack ${visibleActions.length ? "has-actions" : ""}`}>
    {fader}
    {visibleActions.length > 0 && <div className="vertical-touch-fader-actions" style={{ "--fader-action-count": visibleActions.length } as CSSProperties}>{visibleActions.map(({ id, label: actionLabel, ...props }) => <Button type="button" {...props} key={id}>{actionLabel}</Button>)}</div>}
    {inputOpen && <SetValueDialog label={label} value={inputValue} maximum={maximum} offset={directInputOffset} onChange={setInputValue} onFaderChange={(next) => { const entered = next - directInputOffset; setInputValue(String(Number(entered.toFixed(1)))); setLocalValue(next); onChange?.(next); }} onSubmit={submitInput} onClose={()=>setInputOpen(false)}/>}
  </div>;
}

export function TouchValueButton({ label, value, maximum = 100, display, onChange }: Pick<VerticalTouchFaderProps, "label" | "value" | "maximum" | "display" | "onChange">) {
  const [open, setOpen] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const shown = display ?? `${Math.round(value)}%`;
  const show = () => { setInputValue(String(Number(value.toFixed(1)))); setOpen(true); };
  const apply = (next: number) => {
    const clamped = Math.max(0, Math.min(maximum, next));
    setInputValue(String(Number(clamped.toFixed(1))));
    onChange?.(clamped);
  };
  const submit = () => { const next = Number(inputValue); if (Number.isFinite(next)) apply(next); setOpen(false); };
  return <div className="touch-value-button"><Button type="button" onClick={show}><span>{label}</span><strong>{shown}</strong><small>Set value</small></Button>{open && <SetValueDialog label={label} value={inputValue} maximum={maximum} offset={0} onChange={setInputValue} onFaderChange={apply} onSubmit={submit} onClose={()=>setOpen(false)}/>}</div>;
}
