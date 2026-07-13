import { useState, type ReactNode } from "react";
import { Button } from "./controls";
import { Select, TextInput } from "./";

export interface SearchFilter { id: string; label: string; options: string[] }

export function SearchBar({ value, onChange, onSearch, filters = [], values = {}, onFilterChange, placeholder = "Search" }: { value: string; onChange: (value: string) => void; onSearch?: () => void; filters?: SearchFilter[]; values?: Record<string, string>; onFilterChange?: (id: string, value: string) => void; placeholder?: string }) {
  const [open, setOpen] = useState(false);
  const submit = () => onSearch?.();
  return <div className="console-search"><Button className="search-filters" aria-label="Search filters" onClick={() => setOpen(true)}>☷</Button><div><TextInput clearable aria-label="Search" value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") submit(); }}/><Button aria-label="Search" onClick={submit}>⌕</Button></div>{open && <div className="stacked-modal-layer"><section className="nested-modal search-filter-modal" role="dialog" aria-modal="true"><header><h3>Search filters</h3><Button onClick={() => setOpen(false)}>×</Button></header>{filters.map((filter) => <label key={filter.id}>{filter.label}<Select value={values[filter.id] ?? ""} onChange={(event) => onFilterChange?.(filter.id, event.target.value)}><option value="">All</option>{filter.options.map((option) => <option key={option}>{option}</option>)}</Select></label>)}<footer><Button onClick={() => { for (const filter of filters) onFilterChange?.(filter.id, ""); }}>Clear filters</Button><Button onClick={() => { setOpen(false); submit(); }}>Apply</Button></footer></section></div>}</div>;
}

export function SearchActions({ children }: { children: ReactNode }) { return <div className="search-actions">{children}</div>; }
