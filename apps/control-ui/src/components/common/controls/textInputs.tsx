import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type ChangeEvent,
  type InputHTMLAttributes,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import { InputModal } from "./InputModal";
import { Button } from "./foundation";

function emitInputValue(
  input: HTMLInputElement | null,
  next: string,
  onChange?: (event: ChangeEvent<HTMLInputElement>) => void,
  onValueChange?: (value: string) => void,
) {
  if (!input) return;
  input.value = next;
  onValueChange?.(next);
  onChange?.({ target: input, currentTarget: input } as ChangeEvent<HTMLInputElement>);
}

function emitTextAreaValue(
  input: HTMLTextAreaElement | null,
  next: string,
  onChange?: (event: ChangeEvent<HTMLTextAreaElement>) => void,
  onValueChange?: (value: string) => void,
) {
  if (!input) return;
  input.value = next;
  onValueChange?.(next);
  onChange?.({ target: input, currentTarget: input } as ChangeEvent<HTMLTextAreaElement>);
}

function refocus(input: HTMLElement | null) {
  requestAnimationFrame(() => input?.focus());
}

export interface TextInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  clearable?: boolean;
  clearLabel?: string;
  keyboardLabel?: string;
  liveKeyboard?: boolean;
  onValueChange?: (value: string) => void;
  onKeyboardCommit?: (value: string) => void;
  openKeyboardInitially?: boolean;
  secure?: boolean;
}

export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(function TextInput(
  {
    className = "", value, defaultValue, onChange, onValueChange, onKeyboardCommit,
    clearable = false, clearLabel = "Clear input", keyboardLabel,
    liveKeyboard = false, openKeyboardInitially = false, secure = false,
    disabled, readOnly, ...props
  },
  ref,
) {
  const [open, setOpen] = useState(openKeyboardInitially);
  const native = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => native.current!);
  const current = String(value ?? native.current?.value ?? defaultValue ?? "");
  const update = (next: string) => emitInputValue(native.current, next, onChange, onValueChange);
  const close = () => {
    setOpen(false);
    refocus(native.current);
  };
  const commit = (next: string) => {
    update(next);
    close();
    onKeyboardCommit?.(next);
  };
  return <span className="ui-text-control">
    <input {...props} ref={native} type={secure ? "password" : "text"} value={value}
      defaultValue={defaultValue} disabled={disabled} readOnly={readOnly}
      className={`ui-input ${className}`.trim()}
      onChange={(event) => {
        onValueChange?.(event.target.value);
        onChange?.(event);
      }}/>
    {clearable && current && <Button size="compact" iconOnly className="ui-input-clear"
      aria-label={clearLabel} disabled={disabled || readOnly}
      onClick={() => { update(""); native.current?.focus(); }}>×</Button>}
    <Button size="compact" iconOnly className="ui-input-keyboard" aria-label="Open keyboard"
      disabled={disabled || readOnly} onClick={() => setOpen(true)}>
      <span className="ui-keyboard-icon" aria-hidden="true">⌨</span>
    </Button>
    {open && <InputModal kind="text" value={current} secure={secure}
      label={keyboardLabel ?? props["aria-label"]}
      onDraftChange={liveKeyboard ? update : undefined}
      onCommit={commit} onCancel={close}/>}
  </span>;
});

export interface NumberInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  allowDecimal?: boolean;
  showStepButtons?: boolean;
  keyboardLabel?: string;
  onValueChange?: (value: string) => void;
  onKeyboardCommit?: (value: string) => void;
  unit?: ReactNode;
}

function clampNumber(value: number, min: NumberInputProps["min"], max: NumberInputProps["max"]) {
  const lower = min == null ? -Infinity : Number(min);
  const upper = max == null ? Infinity : Number(max);
  return Math.max(lower, Math.min(upper, value));
}

function normalizeNumberText(next: string, allowDecimal: boolean, current: string) {
  const filtered = allowDecimal
    ? next.replace(/[^\d.-]/g, "")
    : next.replace(/[^\d-]/g, "");
  if (filtered === "" || filtered === "-") return filtered;
  const valid = allowDecimal ? /^-?\d*\.?\d*$/.test(filtered) : /^-?\d+$/.test(filtered);
  return valid ? filtered : current;
}

function committedNumberText(
  next: string,
  min: NumberInputProps["min"],
  max: NumberInputProps["max"],
) {
  const parsed = Number(next);
  if (next === "" || next === "-" || Number.isNaN(parsed)) return "";
  return String(clampNumber(parsed, min, max));
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput(
  {
    className = "", value, defaultValue, onChange, onValueChange, onKeyboardCommit,
    allowDecimal = false, showStepButtons = true, keyboardLabel, unit, disabled,
    readOnly, min, max, step = 1, ...props
  },
  ref,
) {
  const [open, setOpen] = useState(false);
  const native = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => native.current!);
  const current = String(value ?? native.current?.value ?? defaultValue ?? "");
  const update = (next: string) => emitInputValue(
    native.current, normalizeNumberText(next, allowDecimal, current), onChange, onValueChange,
  );
  const commit = (next: string) => {
    const committed = committedNumberText(next, min, max);
    update(committed);
    return committed;
  };
  const bump = (direction: -1 | 1) => {
    update(String(clampNumber((Number(current) || 0) + Number(step) * direction, min, max)));
  };
  const close = () => {
    setOpen(false);
    refocus(native.current);
  };
  const commitModal = (next: string) => {
    const committed = commit(next);
    close();
    onKeyboardCommit?.(committed);
  };
  return <span className={`ui-number-control ${showStepButtons ? "with-steppers" : "without-steppers"}`}>
    {showStepButtons && <Button size="compact" iconOnly className="ui-number-minus"
      aria-label="Decrease value" disabled={disabled || readOnly} onClick={() => bump(-1)}>
      <span className="ui-step-icon" aria-hidden="true">−</span>
    </Button>}
    <input {...props} ref={native} type="text" inputMode={allowDecimal ? "decimal" : "numeric"}
      value={value} defaultValue={defaultValue} disabled={disabled} readOnly={readOnly}
      className={`ui-input ${className}`.trim()}
      onChange={(event) => {
        const next = normalizeNumberText(event.target.value, allowDecimal, current);
        if (next !== event.target.value) event.target.value = next;
        onValueChange?.(next);
        onChange?.(event);
      }}
      onBlur={(event) => { commit(event.target.value); props.onBlur?.(event); }}/>
    {showStepButtons && <Button size="compact" iconOnly className="ui-number-plus"
      aria-label="Increase value" disabled={disabled || readOnly} onClick={() => bump(1)}>
      <span className="ui-step-icon" aria-hidden="true">+</span>
    </Button>}
    <Button size="compact" iconOnly className="ui-input-keyboard" aria-label="Open number pad"
      disabled={disabled || readOnly} onClick={() => setOpen(true)}>
      <span className="ui-keyboard-icon" aria-hidden="true">⌨</span>
    </Button>
    {open && <InputModal kind="number" value={current} allowDecimal={allowDecimal}
      label={keyboardLabel ?? props["aria-label"]} unit={unit}
      onCommit={commitModal} onCancel={close}/>}
  </span>;
});

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  allowDecimal?: boolean;
  showStepButtons?: boolean;
}

const WheelSafeRangeInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function WheelSafeRangeInput({ className = "", onWheel: _onWheel, ...props }, ref) {
    const native = useRef<HTMLInputElement>(null);
    useImperativeHandle(ref, () => native.current!);
    useEffect(() => {
      const input = native.current;
      if (!input) return;
      const rejectWheel = (event: WheelEvent) => {
        event.preventDefault();
        input.blur();
      };
      input.addEventListener("wheel", rejectWheel, { passive: false });
      return () => input.removeEventListener("wheel", rejectWheel);
    }, []);
    return <input {...props} ref={native} type="range"
      className={`ui-native-control ${className}`.trim()}/>;
  },
);

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className = "", type = "text", allowDecimal, showStepButtons, ...props }, ref,
) {
  if (type === "number") {
    return <NumberInput {...props} allowDecimal={allowDecimal}
      showStepButtons={showStepButtons} className={className} ref={ref}/>;
  }
  if (type === "range") return <WheelSafeRangeInput {...props} className={className} ref={ref}/>;
  const native = ["checkbox", "radio", "range", "file", "color", "hidden"].includes(type);
  return <input ref={ref} type={type}
    className={`${native ? "ui-native-control" : "ui-input"} ${className}`.trim()}
    {...props}/>;
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function TextArea({ className = "", ...props }, ref) {
    return <textarea ref={ref} className={`ui-textarea ${className}`.trim()} {...props}/>;
  },
);

export interface LargeTextInputProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  keyboardLabel?: string;
  liveKeyboard?: boolean;
  onKeyboardCommit?: (value: string) => void;
  onValueChange?: (value: string) => void;
}

function previousLineTarget(area: HTMLTextAreaElement, position: number, column: number) {
  const lineStart = area.value.lastIndexOf("\n", Math.max(0, position - 1)) + 1;
  if (lineStart === 0) return 0;
  const previousEnd = lineStart - 1;
  const previousStart = area.value.lastIndexOf("\n", Math.max(0, previousEnd - 1)) + 1;
  return previousStart + Math.min(column, previousEnd - previousStart);
}

function nextLineTarget(area: HTMLTextAreaElement, position: number, column: number) {
  const lineEnd = area.value.indexOf("\n", position);
  if (lineEnd < 0) return area.value.length;
  const nextStart = lineEnd + 1;
  const nextEnd = area.value.indexOf("\n", nextStart);
  return nextStart + Math.min(column, (nextEnd < 0 ? area.value.length : nextEnd) - nextStart);
}

function moveTextAreaCursor(area: HTMLTextAreaElement, direction: -1 | 1) {
  const position = area.selectionStart;
  const lineStart = area.value.lastIndexOf("\n", Math.max(0, position - 1)) + 1;
  const column = position - lineStart;
  const target = direction < 0
    ? previousLineTarget(area, position, column)
    : nextLineTarget(area, position, column);
  area.focus();
  area.setSelectionRange(target, target);
  const lineHeight = Number.parseFloat(getComputedStyle(area).lineHeight) || 22;
  area.scrollBy({ top: direction * lineHeight * 2, behavior: "smooth" });
}

export const LargeTextInput = forwardRef<HTMLTextAreaElement, LargeTextInputProps>(
  function LargeTextInput({
    className = "", value, defaultValue, onChange, onValueChange, onKeyboardCommit,
    keyboardLabel, liveKeyboard = false, disabled, readOnly, ...props
  }, ref) {
    const [open, setOpen] = useState(false);
    const native = useRef<HTMLTextAreaElement>(null);
    useImperativeHandle(ref, () => native.current!);
    const current = String(value ?? native.current?.value ?? defaultValue ?? "");
    const update = (next: string) => emitTextAreaValue(native.current, next, onChange, onValueChange);
    const close = () => { setOpen(false); refocus(native.current); };
    const commit = (next: string) => { update(next); close(); onKeyboardCommit?.(next); };
    return <span className="ui-large-text-control">
      <textarea {...props} ref={native} value={value} defaultValue={defaultValue}
        disabled={disabled} readOnly={readOnly} className={`ui-textarea ${className}`.trim()}
        onChange={(event) => { onValueChange?.(event.target.value); onChange?.(event); }}/>
      <Button size="compact" iconOnly className="ui-large-text-up"
        aria-label="Move cursor and scroll up" disabled={disabled}
        onClick={() => native.current && moveTextAreaCursor(native.current, -1)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 15 7-7 7 7"/></svg>
      </Button>
      <Button size="compact" iconOnly className="ui-large-text-down"
        aria-label="Move cursor and scroll down" disabled={disabled}
        onClick={() => native.current && moveTextAreaCursor(native.current, 1)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 9 7 7 7-7"/></svg>
      </Button>
      <Button size="compact" iconOnly className="ui-large-text-keyboard"
        aria-label="Open keyboard" disabled={disabled || readOnly} onClick={() => setOpen(true)}>
        <span className="ui-keyboard-icon" aria-hidden="true">⌨</span>
      </Button>
      {open && <InputModal kind="multiline" value={current}
        label={keyboardLabel ?? props["aria-label"]}
        onDraftChange={liveKeyboard ? update : undefined}
        onCommit={commit} onCancel={close}/>}
    </span>;
  },
);
