import { type FocusEvent } from "react";
import { NumberInput, TextInput } from "../common";

function selectAll(event: FocusEvent<HTMLInputElement>) { event.currentTarget.select(); }

export function ConsoleTextField({ value, onChange, autoFocus, label }: { value: string; onChange: (value: string) => void; autoFocus?: boolean; label?: string }) {
  return <TextInput className="console-field-input" aria-label={label} keyboardLabel={label} autoFocus={autoFocus} value={value} onFocus={selectAll} onChange={(event) => onChange(event.target.value)}/>;
}

export function ConsoleNumberField({ value, onChange, label, allowDecimal = false }: { value: string; onChange: (value: string) => void; label?: string; allowDecimal?: boolean }) {
  return <NumberInput aria-label={label} keyboardLabel={label} allowDecimal={allowDecimal} value={value} onFocus={selectAll} onChange={(event) => onChange(event.target.value)}/>;
}

export function parsePatchAddress(value: string): { universe: number; address: number } | null {
  const match = value.trim().match(/^(\d+)\.(\d+)$/); if (!match) return null;
  const universe = Number(match[1]), address = Number(match[2]);
  return universe >= 1 && address >= 1 && address <= 512 ? { universe, address } : null;
}
