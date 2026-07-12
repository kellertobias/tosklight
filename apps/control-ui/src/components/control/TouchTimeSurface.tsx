import type { ChangeEvent } from "react";
export function TouchTimeSurface({ label, value, display, maximum, onChange }: { label: string; value: number; display: string; maximum: number; onChange: (value: number) => void }) {
  return <label className="touch-time-surface" style={{ "--level": `${value / maximum * 100}%` } as React.CSSProperties}><span>{label}</span><strong>{display}</strong><input aria-label={label} type="range" min="0" max={maximum} step="0.1" value={value} onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(Number(event.target.value))}/></label>;
}
