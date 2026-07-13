import { useEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { Button, Input } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function VerticalTouchFader({ label, value, maximum = 100, display, disabled = false, accentColor, mode, directInput = false, onChange }: { label: string; value: number; maximum?: number; display?: string; disabled?: boolean; accentColor?: string; mode?: string; directInput?: boolean; onChange?: (value: number) => void }) {
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
  const openInput = () => { if (!disabled && directInput) { setInputValue(String(Number(localValue.toFixed(1)))); setInputOpen(true); } };
  const submitInput = () => { const next = Math.max(0, Math.min(maximum, Number(inputValue))); if (Number.isFinite(next)) { setLocalValue(next); onChange?.(next); } setInputOpen(false); };
  const fader = <label onClick={() => hardware && openInput()} className={`vertical-touch-fader ${disabled ? "disabled" : ""} ${directInput ? "direct-input-fader" : ""}`} style={{ "--fader-level": fraction, "--fader-color": accentColor ?? "#176777", "--fader-color-dark": accentColor ? `color-mix(in srgb, ${accentColor} 42%, #081014)` : "#103039" } as CSSProperties}>
    <span>{label}{mode && <small>{mode}</small>}</span><strong>{display === undefined ? `${Math.round(localValue)}%` : display.replace(/^[\d.]+/, String(Math.round(localValue)))}</strong>
    <Input aria-label={label} disabled={disabled || (hardware && directInput)} type="range" min="0" max={maximum} step="0.1" value={localValue} onWheel={(event) => { event.preventDefault(); event.currentTarget.blur(); }} onPointerDown={() => { interacting.current = true; }} onPointerUp={finish} onPointerCancel={finish} onBlur={() => { if (interacting.current) finish(); }} onInput={(event) => emit(Number(event.currentTarget.value))}/>
  </label>;
  return <div className={`vertical-touch-fader-stack ${directInput && !hardware ? "has-set-value" : ""}`}>{fader}{directInput && !hardware && <Button type="button" className="set-value-button" onClick={openInput}>Set value</Button>}{inputOpen && createPortal(<div className="stacked-modal-layer" onClick={(event)=>event.stopPropagation()} onPointerDown={(event)=>event.target===event.currentTarget&&setInputOpen(false)}><section className="nested-modal direct-value-modal" role="dialog" aria-modal="true" aria-label={`${label} value`}><Button className="modal-close" aria-label="Close attribute value" onClick={()=>setInputOpen(false)}>×</Button><h3>{label}</h3><strong>{inputValue || "0"}</strong><ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submitInput} onEscape={()=>setInputOpen(false)} replaceOnFirstInput/></section></div>,document.body)}</div>;
}
