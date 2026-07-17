export type SoftwareKey =
  | "SET" | "GRP" | "CUE" | "UND" | "CLR"
  | "DEL" | "MOV" | "CPY" | "TRU" | "DIV"
  | "BACKSPACE" | "AT" | "ENT" | "PRE" | "REC" | "ESC" | "SHIFT" | "TIME" | "SELECT" | "+" | "-" | "."
  | `${number}`;

export type NumericPadSection = "commands" | "numbers";

export interface NumericPadLayoutItem {
  key: SoftwareKey;
  section: NumericPadSection;
  column: number;
  row: number;
  rowSpan?: number;
}

// Shared physical layout for the software number block and attached/simulated desks.
// Number-section columns retain their full-surface positions so the gap between the
// two blocks remains explicit in layout tests and hardware renderers.
export const numericPadLayout: NumericPadLayoutItem[] = [
  { key: "DEL", section: "commands", column: 1, row: 2 },
  { key: "CLR", section: "commands", column: 2, row: 2 },
  { key: "MOV", section: "commands", column: 1, row: 3 },
  { key: "BACKSPACE", section: "commands", column: 2, row: 3 },
  { key: "CPY", section: "commands", column: 1, row: 4 },
  { key: "UND", section: "commands", column: 2, row: 4 },
  { key: "SET", section: "commands", column: 1, row: 5 },
  { key: "SHIFT", section: "commands", column: 2, row: 5 },
  { key: "GRP", section: "numbers", column: 4, row: 1 },
  { key: "CUE", section: "numbers", column: 5, row: 1 },
  { key: "TIME", section: "numbers", column: 6, row: 1 },
  { key: "DIV", section: "numbers", column: 7, row: 1 },
  { key: "7", section: "numbers", column: 4, row: 2 },
  { key: "8", section: "numbers", column: 5, row: 2 },
  { key: "9", section: "numbers", column: 6, row: 2 },
  { key: "-", section: "numbers", column: 7, row: 2 },
  { key: "4", section: "numbers", column: 4, row: 3 },
  { key: "5", section: "numbers", column: 5, row: 3 },
  { key: "6", section: "numbers", column: 6, row: 3 },
  { key: "+", section: "numbers", column: 7, row: 3 },
  { key: "1", section: "numbers", column: 4, row: 4 },
  { key: "2", section: "numbers", column: 5, row: 4 },
  { key: "3", section: "numbers", column: 6, row: 4 },
  { key: "TRU", section: "numbers", column: 7, row: 4 },
  { key: ".", section: "numbers", column: 4, row: 5 },
  { key: "0", section: "numbers", column: 5, row: 5 },
  { key: "AT", section: "numbers", column: 6, row: 5 },
  { key: "ENT", section: "numbers", column: 7, row: 5 },
];

const oscActionNames: Partial<Record<SoftwareKey, string>> = {
  BACKSPACE: "backspace",
  ENT: "enter",
  GRP: "group",
  TRU: "thru",
  ".": "dot",
  "+": "plus",
  "-": "minus",
  DEL: "del",
  MOV: "mov",
  CPY: "cpy",
};

export function oscProgrammerActionForKey(key: SoftwareKey): string {
  if (/^\d$/.test(key)) return `digit-${key}`;
  return oscActionNames[key] ?? key.toLowerCase();
}

export function softwareKeyLabel(key: SoftwareKey): string {
  return key === "BACKSPACE" ? "←" : key;
}
