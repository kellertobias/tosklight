import { useEffect, useId, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Button } from "../common/controls/foundation";

const inputStack: string[] = [];

function useModalInput(onKey: (key: string) => void) {
  const id = useId();
  const handler = useRef(onKey);
  const root = useRef<HTMLDivElement>(null);
  handler.current = onKey;
  useEffect(() => {
    inputStack.push(id);
    const keydown = (event: KeyboardEvent) => {
      if (inputStack.at(-1) !== id) return;
      const topLayer = document.querySelector<HTMLElement>(
        '.ui-modal-stack-layer[data-modal-top="true"]',
      );
      if (topLayer && root.current && !topLayer.contains(root.current)) return;
      const key = event.key;
      if (
        key !== "Escape"
        && key !== "Enter"
        && key !== "Backspace"
        && key !== "ArrowLeft"
        && key !== "ArrowRight"
        && key.length !== 1
      ) return;
      event.preventDefault(); event.stopImmediatePropagation(); handler.current(key);
    };
    window.addEventListener("keydown", keydown, true);
    return () => {
      window.removeEventListener("keydown", keydown, true);
      const index = inputStack.lastIndexOf(id);
      if (index >= 0) inputStack.splice(index, 1);
    };
  }, [id]);
  return root;
}

interface ModalInputProps {
  value: string;
  onChange: (value: string) => void;
  onEnter: () => void;
  onEscape: () => void;
  onCaretChange?: (position: number) => void;
}

function useModalCaret(value: string, onCaretChange?: (position: number) => void) {
  const [storedCaret, setCaretState] = useState(value.length);
  const caret = Math.min(storedCaret, value.length);
  const setCaret = (position: number, maximum = value.length) => {
    const next = Math.max(0, Math.min(position, maximum));
    setCaretState(next);
    onCaretChange?.(next);
  };
  return [caret, setCaret] as const;
}

function replaceAtCaret(
  value: string,
  caret: number,
  removeBefore: number,
  insertion: string,
) {
  const start = Math.max(0, caret - removeBefore);
  return {
    value: `${value.slice(0, start)}${insertion}${value.slice(caret)}`,
    caret: start + insertion.length,
  };
}

export function ModalCaretValue({
  value,
  caret,
  placeholder,
  multiline = false,
  secure = false,
  onCaretChange,
  ariaLabel,
}: {
  value: string;
  caret: number;
  placeholder?: string;
  multiline?: boolean;
  secure?: boolean;
  onCaretChange?: (position: number) => void;
  ariaLabel?: string;
}) {
  const position = Math.max(0, Math.min(caret, value.length));
  const display = secure ? "•".repeat(value.length) : value;
  const placeCaret = (event: ReactPointerEvent<HTMLOutputElement>) => {
    if (!onCaretChange) return;
    const element = event.currentTarget;
    const point = document.caretPositionFromPoint?.(event.clientX, event.clientY);
    if (point?.offsetNode && element.contains(point.offsetNode)) {
      const before = document.createRange();
      before.selectNodeContents(element);
      before.setEnd(point.offsetNode, point.offset);
      onCaretChange(Math.min(value.length, before.toString().replace("\u200b", "").length));
      return;
    }
    const box = element.getBoundingClientRect();
    const fraction = box.width ? (event.clientX - box.left) / box.width : 1;
    onCaretChange(Math.round(Math.max(0, Math.min(1, fraction)) * value.length));
  };
  return <output
    className={`modal-caret-value ${multiline ? "multiline" : ""} ${!value ? "is-empty" : ""}`}
    role="textbox"
    aria-label={ariaLabel ?? `Input value ${value || placeholder || "empty"}`}
    aria-placeholder={placeholder}
    aria-multiline={multiline || undefined}
    tabIndex={0}
    onPointerDown={placeCaret}
  >
    {!value && placeholder
      ? <span className="modal-value-placeholder">{placeholder}</span>
      : <span>{display.slice(0, position)}</span>}
    <i aria-hidden="true"/>
    <span>{display.slice(position)}</span>
  </output>;
}

export function ModalNumberInput({
  value,
  onChange,
  onEnter,
  onEscape,
  onCaretChange,
  replaceOnFirstInput = false,
  allowDecimal = true,
  allowThrough = false,
}: ModalInputProps & {
  replaceOnFirstInput?: boolean;
  allowDecimal?: boolean;
  allowThrough?: boolean;
}) {
  const replace = useRef(replaceOnFirstInput);
  const [caret, setCaret] = useModalCaret(value, onCaretChange);
  const update = (next: { value: string; caret: number }) => {
    replace.current = false;
    onChange(next.value);
    setCaret(next.caret, next.value.length);
  };
  const press = (key: string) => {
    if (key === "Escape") return onEscape();
    if (key === "Enter") return onEnter();
    if (key === "ArrowLeft") { replace.current = false; return setCaret(caret - 1); }
    if (key === "ArrowRight") { replace.current = false; return setCaret(caret + 1); }
    if (key === "Backspace" || key === "⌫") {
      return update(replace.current
        ? { value: "", caret: 0 }
        : replaceAtCaret(value, caret, caret > 0 ? 1 : 0, ""));
    }
    if (key === "−" || key === "-") {
      const next = replace.current
        ? { value: "-", caret: 1 }
        : value.startsWith("-")
          ? { value: value.slice(1), caret: Math.max(0, caret - 1) }
          : { value: `-${value || "0"}`, caret: caret + 1 };
      return update(next);
    }
    if (key === "+") {
      return update(value.startsWith("-")
        ? { value: value.slice(1), caret: Math.max(0, caret - 1) }
        : { value, caret });
    }
    if (key === "THRU") {
      const before = value.slice(0, caret);
      const after = value.slice(caret);
      if (
        !allowThrough
        || replace.current
        || !before.trim()
        || !after.trim() && /\bTHRU\s*$/i.test(before)
      ) return;
      return update(replaceAtCaret(value, caret, 0, " THRU "));
    }
    if (/^\d$/.test(key)) {
      return update(replace.current
        ? { value: key, caret: 1 }
        : replaceAtCaret(value, caret, 0, key));
    }
    if (allowDecimal && key === ".") {
      const tokenBefore = value.slice(0, caret).split(/\s+THRU\s+/i).at(-1) ?? "";
      const tokenAfter = value.slice(caret).split(/\s+THRU\s+/i)[0] ?? "";
      if (replace.current) return update({ value: "0.", caret: 2 });
      if (!`${tokenBefore}${tokenAfter}`.includes(".")) {
        return update(replaceAtCaret(value, caret, 0, tokenBefore ? "." : "0."));
      }
    }
  };
  const root = useModalInput(press);
  // Modal number pads keep a fixed five-column geometry. Attribute
  // value dialogs may place an optional touch fader beside this grid.
  const rows = [
    ["ESC", "7", "8", "9", "⌫"],
    ["+", "4", "5", "6", "THRU"],
    ["DIV", "1", "2", "3", "ENTER"],
    ["−", ".", "0", "AT"],
  ];
  return <div ref={root} className="modal-number-input numeric-pad" aria-label="Number input keypad">{rows.flatMap((row, rowIndex) => row.map((key, columnIndex) => {
    if (key === "." && !allowDecimal || key === "THRU" && !allowThrough) return null;
    return <Button
      data-keypad-key={key}
      key={key}
      style={{ gridColumn: columnIndex + 1, gridRow: key === "ENTER" ? `${rowIndex + 1} / span 2` : rowIndex + 1 }}
      onClick={() => press(key === "ENTER" ? "Enter" : key === "ESC" ? "Escape" : key)}
      className={key === "ENTER" ? "enter modal-number-input-enter" : ["ESC", "THRU", "DIV", "AT", "+", "−", "⌫"].includes(key) ? "action" : ""}
    >{key}</Button>;
  }))}
    <Button className="action cursor-left" aria-label="Move cursor left"
      style={{ gridColumn: 2, gridRow: 5 }} onClick={() => press("ArrowLeft")}>←</Button>
    <Button className="action cursor-right" aria-label="Move cursor right"
      style={{ gridColumn: 3, gridRow: 5 }} onClick={() => press("ArrowRight")}>→</Button>
  </div>;
}

const physicalRows = [
  ["Digit1","Digit2","Digit3","Digit4","Digit5","Digit6","Digit7","Digit8","Digit9","Digit0","Minus","Equal"],
  ["KeyQ","KeyW","KeyE","KeyR","KeyT","KeyY","KeyU","KeyI","KeyO","KeyP","BracketLeft","BracketRight"],
  ["KeyA","KeyS","KeyD","KeyF","KeyG","KeyH","KeyJ","KeyK","KeyL","Semicolon","Quote"],
  ["KeyZ","KeyX","KeyC","KeyV","KeyB","KeyN","KeyM","Comma","Period","Slash"],
];

const qwertyValues: Record<string, string> = {
  Digit1:"1",Digit2:"2",Digit3:"3",Digit4:"4",Digit5:"5",Digit6:"6",Digit7:"7",Digit8:"8",Digit9:"9",Digit0:"0",Minus:"-",Equal:"=",
  KeyQ:"Q",KeyW:"W",KeyE:"E",KeyR:"R",KeyT:"T",KeyY:"Y",KeyU:"U",KeyI:"I",KeyO:"O",KeyP:"P",BracketLeft:"[",BracketRight:"]",
  KeyA:"A",KeyS:"S",KeyD:"D",KeyF:"F",KeyG:"G",KeyH:"H",KeyJ:"J",KeyK:"K",KeyL:"L",Semicolon:";",Quote:"'",
  KeyZ:"Z",KeyX:"X",KeyC:"C",KeyV:"V",KeyB:"B",KeyN:"N",KeyM:"M",Comma:",",Period:".",Slash:"/",
};

const shiftedValues: Record<string, string> = {
  Digit1:"!",Digit2:"@",Digit3:"#",Digit4:"$",Digit5:"%",Digit6:"^",Digit7:"&",Digit8:"*",Digit9:"(",Digit0:")",
  Minus:"_",Equal:"+",BracketLeft:"{",BracketRight:"}",Semicolon:":",Quote:'"',Comma:"<",Period:">",Slash:"?",
};

export function fallbackKeyboardLayout(language = "en") {
  if (!language.toLowerCase().startsWith("de")) return qwertyValues;
  return { ...qwertyValues, KeyY: "Z", KeyZ: "Y", Minus: "ß", BracketLeft: "Ü", Semicolon: "Ö", Quote: "Ä", BracketRight: "+" };
}

function displayKey(value: string) {
  return value.length === 1 && value !== "ß" ? value.toLocaleUpperCase() : value;
}

type ShiftState = "inactive" | "one-shot" | "locked";

export function ModalTextKeyboard({
  value,
  onChange,
  onEnter,
  onEscape,
  onCaretChange,
  actionLabel = "Confirm",
  multiline = false,
}: ModalInputProps & { actionLabel?: string; multiline?: boolean }) {
  const [layout, setLayout] = useState<Record<string, string>>(() => fallbackKeyboardLayout(navigator.language));
  const [caret, setCaret] = useModalCaret(value, onCaretChange);
  const [shift, setShift] = useState<ShiftState>("inactive");
  const holdTimer = useRef<number | null>(null);
  const holdLocked = useRef(false);
  const update = (next: { value: string; caret: number }) => {
    onChange(next.value);
    const position = Math.max(0, Math.min(next.caret, next.value.length));
    setCaret(position, next.value.length);
  };
  useEffect(() => {
    let cancelled = false;
    const keyboard = (navigator as Navigator & { keyboard?: { getLayoutMap?: () => Promise<Map<string, string>> } }).keyboard;
    if (keyboard?.getLayoutMap) void keyboard.getLayoutMap().then((map) => {
      if (cancelled) return;
      setLayout(Object.fromEntries(physicalRows.flat().map((code) => [code, map.get(code) ?? fallbackKeyboardLayout(navigator.language)[code] ?? ""] )));
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, []);
  const press = (key: string) => {
    if (key === "Escape") return onEscape();
    if (key === "Enter") return multiline ? update(replaceAtCaret(value, caret, 0, "\n")) : onEnter();
    if (key === "Confirm") return onEnter();
    if (key === "ArrowLeft") return setCaret(caret - 1);
    if (key === "ArrowRight") return setCaret(caret + 1);
    if (key === "Backspace" || key === "⌫") {
      return update(replaceAtCaret(value, caret, caret > 0 ? 1 : 0, ""));
    }
    if (key === "SPACE") return update(replaceAtCaret(value, caret, 0, " "));
    if (key.length === 1) {
      const next = shift === "inactive" ? key : key.length === 1 ? key.toLocaleUpperCase() : key;
      update(replaceAtCaret(value, caret, 0, next));
      if (shift === "one-shot") {
        if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
        holdTimer.current = null;
        setShift("inactive");
      }
    }
  };
  const keyValue = (code: string) => {
    const normal = layout[code] ?? "";
    if (shift === "inactive") return /^[A-Z]$/u.test(normal) ? normal.toLocaleLowerCase() : normal;
    return shiftedValues[code] ?? (normal.length === 1 ? normal.toLocaleUpperCase() : normal);
  };
  const toggleShift = () => setShift((current) => current === "inactive" ? "one-shot" : "inactive");
  const beginShiftHold = () => {
    holdLocked.current = false;
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
    holdTimer.current = window.setTimeout(() => {
      holdLocked.current = true;
      setShift("locked");
    }, 500);
  };
  const endShiftHold = () => {
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
    holdTimer.current = null;
    if (!holdLocked.current) toggleShift();
  };
  useEffect(() => () => {
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
  }, []);
  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (event.key !== "Shift" || event.repeat) return;
      if (shift === "locked") {
        setShift("inactive");
        return;
      }
      setShift("one-shot");
      beginShiftHold();
    };
    const up = (event: KeyboardEvent) => {
      if (event.key !== "Shift") return;
      if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
      holdTimer.current = null;
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    };
  }, [shift]);
  const root = useModalInput(press);
  return <div ref={root} className="modal-text-keyboard" aria-label="Full text keyboard">
    <div className="modal-keyboard-main">
      <div className="modal-keyboard-row row-1"><Button className="escape" onClick={() => press("Escape")}><b>ESC</b><small>Cancel</small></Button>{physicalRows[0].map((code) => <Button key={code} onClick={() => press(keyValue(code))}>{displayKey(keyValue(code))}</Button>)}</div>
      {physicalRows.slice(1).map((row, index) => <div className={`modal-keyboard-row row-${index + 2}`} key={index}>{row.map((code) => <Button key={code} onClick={() => press(keyValue(code))}>{displayKey(keyValue(code))}</Button>)}</div>)}
      <div className="modal-keyboard-row modal-keyboard-bottom">
        <Button
          className={`shift shift-${shift}`}
          aria-label="Shift"
          aria-pressed={shift !== "inactive"}
          data-shift-state={shift}
          onClick={(event) => {
            if (event.detail === 0) toggleShift();
          }}
          onPointerDown={beginShiftHold}
          onPointerUp={endShiftHold}
          onPointerCancel={() => {
            if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
            holdTimer.current = null;
          }}
        >SHIFT</Button>
        {!multiline && <Button className="action backspace" aria-label="Backspace" onClick={() => press("Backspace")}>⌫</Button>}
        <Button className="action cursor-left" aria-label="Move cursor left"
          onClick={() => press("ArrowLeft")}>←</Button>
        <Button className="action cursor-right" aria-label="Move cursor right"
          onClick={() => press("ArrowRight")}>→</Button>
        <span className="modal-keyboard-gap" aria-hidden="true"/>
        <Button className="space" onClick={() => press("SPACE")}>SPACE</Button>
      </div>
    </div>
    <div className={`modal-keyboard-actions ${multiline ? "multiline" : ""}`}>
      {multiline
        ? <><Button className="action backspace" onClick={() => press("Backspace")}><b>⌫</b><small>Backspace</small></Button>
          <Button className="action newline" aria-label="Enter · New line" onClick={() => press("Enter")}><b>ENTER</b><small>New line</small></Button></>
        : <Button className="enter" aria-label={`Enter · ${actionLabel}`} onClick={() => press("Enter")}><b>ENTER</b><small>{actionLabel}</small></Button>}
    </div>
  </div>;
}
