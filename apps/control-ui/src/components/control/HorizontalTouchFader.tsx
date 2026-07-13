import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Input } from "../common";

export function HorizontalTouchFader({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  const [local, setLocal] = useState(value);
  const dragging = useRef(false);
  useEffect(() => { if (!dragging.current) setLocal(value); }, [value]);
  return <label className="horizontal-touch-fader" style={{ "--fader-level": Math.max(0, Math.min(1, local / 100)) } as CSSProperties}>
    <span>{label}</span><strong>{Math.round(local)}%</strong>
    <Input aria-label={label} type="range" min="0" max="100" step="0.1" value={local} onPointerDown={() => { dragging.current = true; }} onInput={(event) => { const next = Number(event.currentTarget.value); setLocal(next); onChange(next); }} onPointerUp={() => { dragging.current = false; onChange(local); }} onPointerCancel={() => { dragging.current = false; onChange(local); }}/>
  </label>;
}
