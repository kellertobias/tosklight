import { useState } from "react";

export function TouchSelect({ label, value, options, onChange }: { label: string; value: number; options: number[]; onChange: (value: number) => void }) {
  const [open, setOpen] = useState(false);
  return <div className="touch-select"><span>{label}</span><button className="touch-select-value" aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen(!open)}>{value}<i>▾</i></button>{open && <div className="touch-select-options" role="listbox" aria-label={label}>{options.map((option) => <button role="option" aria-selected={option === value} className={option === value ? "active" : ""} key={option} onClick={() => { onChange(option); setOpen(false); }}>{option}</button>)}</div>}</div>;
}
