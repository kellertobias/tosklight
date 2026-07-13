import { useRef, useState, type FocusEvent } from "react";
import { ModalNumberInput, ModalTextKeyboard } from "./ModalInputControls";
import { Button, Input } from "../common";

function selectAll(event: FocusEvent<HTMLInputElement>) { event.currentTarget.select(); }

export function ConsoleTextField({ value, onChange, autoFocus, label }: { value: string; onChange: (value: string) => void; autoFocus?: boolean; label?: string }) {
  const [keyboard, setKeyboard] = useState(false);
  return <div className="console-field"><Input aria-label={label} autoFocus={autoFocus} value={value} onFocus={selectAll} onChange={(event) => onChange(event.target.value)}/><Button aria-label="Open keyboard" onClick={() => setKeyboard(true)}>⌨</Button>{keyboard && <div className="stacked-modal-layer"><section className="nested-modal keyboard-modal"><Input autoFocus value={value} onChange={(event) => onChange(event.target.value)}/><ModalTextKeyboard value={value} onChange={onChange} onEnter={() => setKeyboard(false)} onEscape={() => setKeyboard(false)}/></section></div>}</div>;
}

export function ConsoleNumberField({ value, onChange, label, allowDecimal = false }: { value: string; onChange: (value: string) => void; label?: string; allowDecimal?: boolean }) {
  const [pad, setPad] = useState(false); const input = useRef<HTMLInputElement>(null);
  return <div className="console-field"><Input ref={input} aria-label={label} inputMode={allowDecimal ? "decimal" : "numeric"} value={value} onFocus={selectAll} onChange={(event) => onChange(allowDecimal ? event.target.value.replace(/[^\d.-]/g, "") : event.target.value.replace(/\D/g, ""))}/><Button aria-label="Open number pad" onClick={() => setPad(true)}>▦</Button>{pad && <div className="stacked-modal-layer"><section className="nested-modal number-field-modal"><Input value={value} readOnly/><ModalNumberInput value={value} onChange={onChange} onEnter={() => setPad(false)} onEscape={() => setPad(false)}/></section></div>}</div>;
}

export function parsePatchAddress(value: string): { universe: number; address: number } | null {
  const match = value.trim().match(/^(\d+)\.(\d+)$/); if (!match) return null;
  const universe = Number(match[1]), address = Number(match[2]);
  return universe >= 1 && address >= 1 && address <= 512 ? { universe, address } : null;
}
