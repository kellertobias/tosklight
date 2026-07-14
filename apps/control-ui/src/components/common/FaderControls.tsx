import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { FormField, Input } from "./controls";

export interface HorizontalFaderProps {
  label: string;
  value: number;
  minimum?: number;
  maximum?: number;
  step?: number;
  display?: ReactNode;
  disabled?: boolean;
  accentColor?: string;
  className?: string;
  showLabel?: boolean;
  onChange: (value: number) => void;
}

export function HorizontalFader({ label, value, minimum = 0, maximum = 100, step = 0.1, display, disabled = false, accentColor, className = "", showLabel = true, onChange }: HorizontalFaderProps) {
  const [local, setLocal] = useState(value);
  const dragging = useRef(false);
  const latest = useRef(value);
  useEffect(() => { if (!dragging.current) { setLocal(value); latest.current = value; } }, [value]);
  const finish = () => { dragging.current = false; onChange(latest.current); };
  const span = maximum - minimum;
  const fraction = span > 0 ? Math.max(0, Math.min(1, (local - minimum) / span)) : 0;
  return <label className={`horizontal-touch-fader ${disabled ? "disabled" : ""} ${className}`.trim()} style={{ "--fader-level": fraction, "--fader-color": accentColor ?? "#176777", "--fader-color-dark": accentColor ? `color-mix(in srgb, ${accentColor} 42%, #081014)` : "#103039" } as CSSProperties}>
    {showLabel && <span>{label}</span>}<strong>{display ?? `${Math.round(local)}%`}</strong>
    <Input aria-label={label} disabled={disabled} type="range" min={minimum} max={maximum} step={step} value={local} onPointerDown={() => { dragging.current = true; }} onInput={(event) => { const next = Number(event.currentTarget.value); latest.current = next; setLocal(next); onChange(next); }} onPointerUp={finish} onPointerCancel={finish} onBlur={() => { if (dragging.current) finish(); }}/>
  </label>;
}

export function HorizontalFaderField({ fieldLabel, description, error, labelPlacement, ...props }: HorizontalFaderProps & {
  fieldLabel?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  labelPlacement?: "side" | "top";
}) {
  return <FormField label={fieldLabel ?? props.label} description={description} error={error} labelPlacement={labelPlacement}>
    <HorizontalFader {...props} showLabel={false}/>
  </FormField>;
}

/** Compatibility name for existing operator surfaces. */
export const HorizontalTouchFader = HorizontalFader;
