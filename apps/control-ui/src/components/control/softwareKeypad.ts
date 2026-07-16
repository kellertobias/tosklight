import { removeCommandToken } from "./commandLineEditing";

export type SoftwareKey =
  | "SET" | "GRP" | "CUE" | "UND" | "CLR"
  | "DEL" | "MOV" | "CPY" | "TRU" | "DIV"
  | "BACKSPACE" | "AT" | "ENT" | "PRE" | "REC" | "ESC" | "SHIFT" | "TIME" | "SELECT" | "+" | "-" | "."
  | `${number}`;

export const softwareKeypadRows: SoftwareKey[][] = [
  ["SET", "GRP", "CUE", "UND", "CLR"],
  ["DEL", "7", "8", "9", "+"],
  ["MOV", "4", "5", "6", "TRU"],
  ["CPY", "1", "2", "3", "DIV"],
  ["BACKSPACE", "0", ".", "AT", "ENT"],
  ["SHIFT", "TIME", "-"],
];

export type CommandTargetMode = "FIXTURE" | "GROUP";

export interface TargetedCommandEdit {
  command: string;
  execute: boolean;
  pristine: boolean;
}

export function commandTargetAfterEnter(
  command: string,
  target: CommandTargetMode,
  pristine: boolean,
): CommandTargetMode | null {
  if (pristine || command.trim().toUpperCase() !== "GROUP") return null;
  return target === "GROUP" ? "FIXTURE" : "GROUP";
}

export function defaultCommandLine(target: CommandTargetMode): string {
  return target;
}

function shortTarget(target: CommandTargetMode): "F" | "G" {
  return target === "FIXTURE" ? "F" : "G";
}

export function softwareKeyFromKeyboard(
  event: Pick<KeyboardEvent, "code" | "key" | "shiftKey">,
  regularNumbers: boolean,
): SoftwareKey | null {
  if (/^Numpad\d$/.test(event.code)) return event.code.slice(-1) as SoftwareKey;
  if (regularNumbers && /^Digit\d$/.test(event.code) && !event.shiftKey)
    return event.code.slice(-1) as SoftwareKey;
  if (event.code === "NumpadDecimal" || event.code === "Period" && !event.shiftKey) return ".";
  if (event.code === "NumpadAdd") return "+";
  if (event.code === "NumpadSubtract") return "-";
  if (event.code === "Escape") return "ESC";
  if (event.code === "Backspace") return "BACKSPACE";
  if (event.code === "Enter" || event.code === "NumpadEnter") return "ENT";
  if (event.code === "Delete") return "CLR";
  if (event.code === "Home") return "SET";
  if (event.code === "End") return "REC";
  if (event.shiftKey && event.key.toLowerCase() === "z") return "SELECT";

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

export function editTargetedCommandWithSoftwareKey(
  command: string,
  key: SoftwareKey,
  target: CommandTargetMode,
  pristine: boolean,
): TargetedCommandEdit {
  if (key === "BACKSPACE") {
    if (pristine) return { command: target, execute: false, pristine: true };
    const next = removeCommandToken(command);
    return next
      ? { command: next, execute: false, pristine: false }
      : { command: target, execute: false, pristine: true };
  }
  if (key === "SHIFT") return { command, execute: false, pristine };

  if (pristine) {
    if (/^\d$/.test(key)) return { command: `${shortTarget(target)}${key}`, execute: false, pristine: false };
    const root = ({
      GRP: "GROUP", CUE: "CUE", DEL: "DELETE", MOV: "MOVE", CPY: "COPY",
      SET: "SET", AT: "AT", TIME: "TIME", SELECT: "SELECT", "+": "+", "-": "-", ".": ".",
    } as Partial<Record<SoftwareKey, string>>)[key];
    if (root) return { command: root, execute: false, pristine: false };
  }

  const selectionCommand = /^\s*(?:F\d|G\d|FIXTURE\b|GROUP\b|DEGRP\b)/i.test(command);
  if (key === "GRP" && selectionCommand && /(?:\+|-)\s*$/.test(command)) {
    const override = target === "GROUP" ? "F" : "G";
    return { command: `${command.trimEnd()} ${override}`, execute: false, pristine: false };
  }
  if (key === "GRP" && /(?:^|\s)(?:GROUP|G|F)\s*$/i.test(command)) {
    return { command: command.replace(/(?:GROUP|G|F)\s*$/i, "DEGRP"), execute: false, pristine: false };
  }
  if (key === "AT" && /(?:^|\s)AT\s*$/i.test(command)) {
    return { command: command.replace(/AT\s*$/i, "AT FULL"), execute: true, pristine: false };
  }
  if (key === "." && /\.\s*$/.test(command)) {
    return { command: `${command.replace(/\.\s*$/, "").trimEnd()} AT 0`, execute: true, pristine: false };
  }
  if (key === "TIME" && /(?:^|\s)TIME\s*$/i.test(command)) {
    return { command: command.replace(/TIME\s*$/i, "DELAY "), execute: false, pristine: false };
  }
  const token = ({
    GRP: "GROUP", CUE: "CUE", DEL: "DELETE", MOV: "MOVE", CPY: "COPY",
    TRU: "THRU", DIV: "DIV", SET: "SET", AT: "AT", TIME: "TIME", SELECT: "SELECT", "+": "+", "-": "-",
  } as Partial<Record<SoftwareKey, string>>)[key] ?? key;
  const spaced = ["GROUP", "CUE", "DELETE", "MOVE", "COPY", "THRU", "DIV", "SET", "AT", "TIME", "SELECT", "+", "-"].includes(token);
  if (/^\d$/.test(token) && /^\s*GROUP\s*$/i.test(command)) {
    return { command: `G${token}`, execute: false, pristine: false };
  }
  const selectionContinuation = (selectionCommand || /^\s*\+\s*$/.test(command))
    && /(?:\+|-)\s*$/.test(command)
    && !/\bAT\b[^]*$/i.test(command);
  const shortPrefixAwaitingNumber = /^\d$/.test(token) && /(?:^|\s)[FG]$/i.test(command);
  const digitAfterWord = /^\d$/.test(token) && /[A-EH-Z]$/i.test(command);
  const nextToken = /^\d$/.test(token) && selectionContinuation
    ? `${shortTarget(target)}${token}`
    : shortPrefixAwaitingNumber ? token
    : digitAfterWord ? ` ${token}` : token;
  return {
    command: `${command}${spaced ? ` ${nextToken} ` : nextToken}`.replace(/\s+/g, " ").trimStart(),
    execute: false,
    pristine: false,
  };
}

export function editCommandWithSoftwareKey(command: string, key: SoftwareKey) {
  const { pristine: _pristine, ...edit } = editTargetedCommandWithSoftwareKey(command, key, "FIXTURE", false);
  return edit;
}
