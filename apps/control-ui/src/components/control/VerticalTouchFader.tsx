import { useEffect, useRef, useState, type CSSProperties } from "react";

export function VerticalTouchFader({ label, value, maximum = 100, display, disabled = false, accentColor, mode, onChange }: { label: string; value: number; maximum?: number; display?: string; disabled?: boolean; accentColor?: string; mode?: string; onChange?: (value: number) => void }) {
  const [localValue, setLocalValue] = useState(value);
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
  return <label className={`vertical-touch-fader ${disabled ? "disabled" : ""}`} style={{ "--fader-level": fraction, "--fader-color": accentColor ?? "#176777", "--fader-color-dark": accentColor ? `color-mix(in srgb, ${accentColor} 42%, #081014)` : "#103039" } as CSSProperties}>
    <span>{label}{mode && <small>{mode}</small>}</span><strong>{display === undefined ? `${Math.round(localValue)}%` : display.replace(/^[\d.]+/, String(Math.round(localValue)))}</strong>
    <input aria-label={label} disabled={disabled} type="range" min="0" max={maximum} step="0.1" value={localValue} onPointerDown={() => { interacting.current = true; }} onPointerUp={finish} onPointerCancel={finish} onBlur={() => { if (interacting.current) finish(); }} onInput={(event) => emit(Number(event.currentTarget.value))}/>
  </label>;
}
