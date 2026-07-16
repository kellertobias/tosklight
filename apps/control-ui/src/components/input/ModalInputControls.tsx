import { useEffect, useId, useRef, useState } from "react";
import { Button } from "../common";

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
      const layers = [...document.querySelectorAll<HTMLElement>(".modal-backdrop,.stacked-modal-layer")];
      const topLayer = layers.at(-1);
      if (topLayer && root.current && !topLayer.contains(root.current)) return;
      const key = event.key;
      if (key !== "Escape" && key !== "Enter" && key !== "Backspace" && key.length !== 1) return;
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

export function ModalNumberInput({ value, onChange, onEnter, onEscape, replaceOnFirstInput = false, allowDecimal = true }: { value: string; onChange: (value: string) => void; onEnter: () => void; onEscape: () => void; replaceOnFirstInput?: boolean; allowDecimal?: boolean }) {
  const replace = useRef(replaceOnFirstInput);
  const press = (key: string) => {
    if (key === "Escape") return onEscape();
    if (key === "Enter") return onEnter();
    if (key === "Backspace" || key === "←") { const next = replace.current ? "" : value.slice(0, -1); replace.current = false; return onChange(next); }
    if (key === "−" || key === "-") { const next = replace.current ? "-" : value.startsWith("-") ? value.slice(1) : `-${value || "0"}`; replace.current = false; return onChange(next); }
    if (key === "+") { replace.current = false; return onChange(value.startsWith("-") ? value.slice(1) : value); }
    if (key === "THRU") return;
    if (/^\d$/.test(key)) { const next = replace.current ? key : value + key; replace.current = false; return onChange(next); }
    if (allowDecimal && key === "." && (replace.current || !value.includes("."))) { const next = replace.current ? "0." : `${value || "0"}.`; replace.current = false; onChange(next); }
  };
  const root = useModalInput(press);
  // Modal number pads keep a fixed five-column, four-row geometry. Attribute
  // value dialogs may place an optional touch fader beside this grid.
  const rows = [
    ["ESC", "7", "8", "9", "←"],
    ["+", "4", "5", "6", "THRU"],
    ["DIV", "1", "2", "3", "ENTER"],
    ["−", ".", "0", "AT"],
  ];
  return <div ref={root} className="modal-number-input numeric-pad" aria-label="Number input keypad">{rows.flatMap((row, rowIndex) => row.map((key, columnIndex) => <Button
    data-keypad-key={key}
    key={key}
    style={{ gridColumn: columnIndex + 1, gridRow: key === "ENTER" ? `${rowIndex + 1} / span 2` : rowIndex + 1 }}
    onClick={() => press(key === "ENTER" ? "Enter" : key === "ESC" ? "Escape" : key)}
    className={key === "ENTER" ? "enter modal-number-input-enter" : ["ESC", "THRU", "DIV", "AT", "+", "−", "←"].includes(key) ? "action" : ""}
  >{key}</Button>))}</div>;
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

export function fallbackKeyboardLayout(language = "en") {
  if (!language.toLowerCase().startsWith("de")) return qwertyValues;
  return { ...qwertyValues, KeyY: "Z", KeyZ: "Y", Minus: "ß", BracketLeft: "Ü", Semicolon: "Ö", Quote: "Ä", BracketRight: "+" };
}

function displayKey(value: string) {
  return value.length === 1 && value !== "ß" ? value.toLocaleUpperCase() : value;
}

export function ModalTextKeyboard({ value, onChange, onEnter, onEscape, actionLabel = "Confirm" }: { value: string; onChange: (value: string) => void; onEnter: () => void; onEscape: () => void; actionLabel?: string }) {
  const [layout, setLayout] = useState<Record<string, string>>(() => fallbackKeyboardLayout(navigator.language));
  useEffect(() => {
    let cancelled = false;
    const keyboard = (navigator as Navigator & { keyboard?: { getLayoutMap?: () => Promise<Map<string, string>> } }).keyboard;
    if (keyboard?.getLayoutMap) void keyboard.getLayoutMap().then((map) => {
      if (cancelled) return;
      setLayout(Object.fromEntries(physicalRows.flat().map((code) => [code, displayKey(map.get(code) ?? fallbackKeyboardLayout(navigator.language)[code] ?? "")] )));
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, []);
  const press = (key: string) => {
    if (key === "Escape") return onEscape();
    if (key === "Enter") return onEnter();
    if (key === "Backspace" || key === "←") return onChange(value.slice(0, -1));
    if (key === "SPACE") return onChange(`${value} `);
    if (key.length === 1) onChange(value + key);
  };
  const root = useModalInput(press);
  return <div ref={root} className="modal-text-keyboard" aria-label="Full text keyboard">
    <div className="modal-keyboard-main">
      <div className="modal-keyboard-row row-1"><Button className="escape" onClick={() => press("Escape")}><b>ESC</b><small>Cancel</small></Button>{physicalRows[0].map((code) => <Button key={code} onClick={() => press(layout[code])}>{displayKey(layout[code])}</Button>)}</div>
      {physicalRows.slice(1).map((row, index) => <div className={`modal-keyboard-row row-${index + 2}`} key={index}>{row.map((code) => <Button key={code} onClick={() => press(layout[code])}>{displayKey(layout[code])}</Button>)}</div>)}
      <div className="modal-keyboard-row modal-keyboard-bottom"><Button className="space" onClick={() => press("SPACE")}>SPACE</Button></div>
    </div>
    <div className="modal-keyboard-actions"><Button className="action backspace" onClick={() => press("Backspace")}><b>⌫</b><small>Backspace</small></Button><Button className="enter" aria-label={`Enter · ${actionLabel}`} onClick={() => press("Enter")}><b>ENTER</b><small>{actionLabel}</small></Button></div>
  </div>;
}
