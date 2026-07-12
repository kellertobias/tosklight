import { useState, type ReactNode } from "react";

export interface SearchFilter { id: string; label: string; options: string[] }

export function SearchBar({ value, onChange, onSearch, filters = [], values = {}, onFilterChange, placeholder = "Search" }: { value: string; onChange: (value: string) => void; onSearch?: () => void; filters?: SearchFilter[]; values?: Record<string, string>; onFilterChange?: (id: string, value: string) => void; placeholder?: string }) {
  const [open, setOpen] = useState(false);
  const submit = () => onSearch?.();
  return <div className="console-search"><button className="search-filters" aria-label="Search filters" onClick={() => setOpen(true)}>☷</button><div><input type="search" value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") submit(); }}/><button aria-label="Search" onClick={submit}>⌕</button></div>{open && <div className="stacked-modal-layer"><section className="nested-modal search-filter-modal" role="dialog" aria-modal="true"><header><h3>Search filters</h3><button onClick={() => setOpen(false)}>×</button></header>{filters.map((filter) => <label key={filter.id}>{filter.label}<select value={values[filter.id] ?? ""} onChange={(event) => onFilterChange?.(filter.id, event.target.value)}><option value="">All</option>{filter.options.map((option) => <option key={option}>{option}</option>)}</select></label>)}<footer><button onClick={() => { for (const filter of filters) onFilterChange?.(filter.id, ""); }}>Clear filters</button><button onClick={() => { setOpen(false); submit(); }}>Apply</button></footer></section></div>}</div>;
}

export function SearchActions({ children }: { children: ReactNode }) { return <div className="search-actions">{children}</div>; }
