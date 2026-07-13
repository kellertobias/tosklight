import { removeCommandToken } from "./commandLineEditing";

export type SoftwareKey =
  | "SET" | "GRP" | "CUE" | "UND" | "CLR"
  | "DEL" | "MOV" | "CPY" | "TRU" | "DIV"
  | "BACKSPACE" | "AT" | "ENT" | "PRE" | "REC" | "ESC" | "+" | "."
  | `${number}`;

export const softwareKeypadRows: SoftwareKey[][] = [
  ["SET", "GRP", "CUE", "UND", "CLR"],
  ["DEL", "7", "8", "9", "+"],
  ["MOV", "4", "5", "6", "TRU"],
  ["CPY", "1", "2", "3", "DIV"],
  ["BACKSPACE", "0", ".", "AT", "ENT"],
];

export function softwareKeyFromKeyboard(
  event: Pick<KeyboardEvent, "code" | "key" | "shiftKey">,
  regularNumbers: boolean,
): SoftwareKey | null {
  if (/^Numpad\d$/.test(event.code)) return event.code.slice(-1) as SoftwareKey;
  if (regularNumbers && /^Digit\d$/.test(event.code) && !event.shiftKey)
    return event.code.slice(-1) as SoftwareKey;
  if (event.code === "NumpadDecimal" || event.code === "Period" && !event.shiftKey) return ".";
  if (event.code === "NumpadAdd") return "+";
  if (event.code === "Escape") return "ESC";
  if (event.code === "Backspace") return "BACKSPACE";
  if (event.code === "Enter" || event.code === "NumpadEnter") return "ENT";
  if (event.code === "Delete") return "CLR";
  if (event.code === "Home") return "SET";
  if (event.code === "End") return "REC";

  // Physical positions on a German keyboard. Using code keeps the shortcuts
  // stable when the browser reports shifted glyphs such as *, ?, or °.
  if (event.code === "BracketRight") return event.shiftKey ? "CPY" : "+";
  if (event.code === "Minus") return event.shiftKey ? "CUE" : "TRU";
  if (event.code === "Backquote") return event.shiftKey ? "GRP" : "PRE";
  if (event.code === "Equal") return event.shiftKey ? "DEL" : "DIV";
  if (event.code === "Backslash") return event.shiftKey ? "MOV" : "AT";

  // Fallbacks help browsers that do not expose a useful physical key code.
  if (!event.shiftKey && event.key === "+") return "+";
  if (!event.shiftKey && event.key === "ß") return "TRU";
  if (!event.shiftKey && event.key === "^") return "PRE";
  if (!event.shiftKey && event.key === "´") return "DIV";
  if (!event.shiftKey && event.key === "#") return "AT";
  if (event.shiftKey && event.key === "?") return "CUE";
  if (event.shiftKey && event.key === "*") return "CPY";
  if (event.shiftKey && event.key === "'") return "MOV";
  if (event.shiftKey && event.key === "°") return "GRP";
  if (event.shiftKey && event.key === "`") return "DEL";
  return null;
}

export function editCommandWithSoftwareKey(command: string, key: SoftwareKey) {
  if (key === "BACKSPACE") return { command: removeCommandToken(command), execute: false };
  if (key === "AT" && /(?:^|\s)AT\s*$/i.test(command)) {
    return { command: command.replace(/AT\s*$/i, "AT FULL"), execute: true };
  }
  if (key === "." && /\.\s*$/.test(command)) {
    return { command: `${command.replace(/\.\s*$/, "").trimEnd()} AT 0`, execute: true };
  }
  const token = ({
    GRP: "GROUP", CUE: "CUE", DEL: "DELETE", MOV: "MOVE", CPY: "COPY",
    TRU: "THRU", DIV: "DIV", SET: "SET", AT: "AT", "+": "+",
  } as Partial<Record<SoftwareKey, string>>)[key] ?? key;
  const spaced = ["GROUP", "CUE", "DELETE", "MOVE", "COPY", "THRU", "DIV", "SET", "AT", "+"].includes(token);
  return {
    command: `${command}${spaced ? ` ${token} ` : token}`.replace(/\s+/g, " ").trimStart(),
    execute: false,
  };
}
