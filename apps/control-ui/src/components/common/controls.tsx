import {
  Children,
  createContext,
  forwardRef,
  isValidElement,
  useContext,
  useEffect,
  useId,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type ChangeEvent,
  type CSSProperties,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import { ModalNumberInput, ModalTextKeyboard } from "../input/ModalInputControls";
import { ModalTitleBar } from "./ModalTitleBar";

export type ControlSize = "default" | "compact";
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "success" | "warning";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ControlSize;
  active?: boolean;
  loading?: boolean;
  fullWidth?: boolean;
  iconOnly?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "default", active = false, loading = false, fullWidth = false, iconOnly = false, className = "", disabled, children, type = "button", ...props },
  ref,
) {
  const arrowOnly = typeof children === "string" && /^[←→▲▼‹›]$/.test(children.trim());
  return <button ref={ref} type={type} disabled={disabled || loading} aria-busy={loading || undefined} className={`ui-button ui-${variant} ui-${size} ${active ? "is-active" : ""} ${fullWidth ? "is-full-width" : ""} ${iconOnly ? "is-icon-only" : ""} ${arrowOnly ? "is-arrow-only" : ""} ${className}`.trim()} {...props}>{loading ? <><span className="ui-spinner" aria-hidden="true"/>Loading</> : children}</button>;
});

type LabelPlacement = "side" | "top";
const FormLayoutContext = createContext<LabelPlacement>("top");

export function FormLayout({ labelPlacement = "top", columns = 1, minColumnWidth = 240, labelWidth = 150, className = "", children }: {
  labelPlacement?: LabelPlacement;
  columns?: number;
  minColumnWidth?: number;
  labelWidth?: number;
  className?: string;
  children: ReactNode;
}) {
  const style = { "--form-columns": columns, "--form-column-min": `${minColumnWidth}px`, "--form-label-width": `${labelWidth}px` } as CSSProperties;
  return <FormLayoutContext.Provider value={labelPlacement}><div className={`ui-form-layout labels-${labelPlacement} ${className}`.trim()} style={style}>{children}</div></FormLayoutContext.Provider>;
}

export function FormField({ label, description, error, required, htmlFor, children, className = "", labelPlacement }: {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  required?: boolean;
  htmlFor?: string;
  children: ReactNode;
  className?: string;
  labelPlacement?: LabelPlacement;
}) {
  const inherited = useContext(FormLayoutContext);
  const placement = labelPlacement ?? inherited;
  return <div className={`ui-form-field labels-${placement} ${error ? "has-error" : ""} ${className}`.trim()}>
    {label && <label htmlFor={htmlFor}>{label}{required && <span aria-hidden="true"> *</span>}</label>}
    <div className="ui-form-control">{children}</div>
    {description && !error && <small>{description}</small>}
    {error && <small className="ui-field-error" role="alert">{error}</small>}
  </div>;
}

/** Compatibility alias for existing callers. New form code should use FormField. */
export const Field = FormField;

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

function InputModal({ kind, value, allowDecimal = false, secure = false, label, unit, onCommit, onCancel }: {
  kind: "text" | "number";
  value: string;
  allowDecimal?: boolean;
  secure?: boolean;
  label?: string;
  unit?: ReactNode;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState(value);
  return createPortal(<div className="stacked-modal-layer ui-input-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
    <section className={`nested-modal ${kind === "text" ? "keyboard-modal" : "number-field-modal"}`} role="dialog" aria-modal="true" aria-label={label ?? (kind === "text" ? "Text input" : "Number input")}>
      <ModalTitleBar title={label ?? (kind === "text" ? "Text input" : "Number input")} closeLabel="Close input" onClose={onCancel}/>
      {kind === "number" && unit ? <div className="modal-number-value"><input className="ui-input" type="text" aria-label={`${label ?? kind} value`} value={draft} readOnly/><span aria-label="Unit">{unit}</span></div> : <input className="ui-input" type={secure ? "password" : "text"} aria-label={`${label ?? kind} value`} value={draft} readOnly/>}
      {kind === "text"
        ? <ModalTextKeyboard value={draft} onChange={setDraft} onEnter={() => onCommit(draft)} onEscape={onCancel}/>
        : <ModalNumberInput value={draft} allowDecimal={allowDecimal} onChange={setDraft} onEnter={() => onCommit(draft)} onEscape={onCancel}/>
      }
    </section>
  </div>, document.body);
}

export interface TextInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  clearable?: boolean;
  keyboardLabel?: string;
  onValueChange?: (value: string) => void;
  onKeyboardCommit?: (value: string) => void;
  secure?: boolean;
}

export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(function TextInput(
  { className = "", value, defaultValue, onChange, onValueChange, onKeyboardCommit, clearable = false, keyboardLabel, secure = false, disabled, readOnly, ...props },
  ref,
) {
  const [open, setOpen] = useState(false);
  const native = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => native.current!);
  const current = String(value ?? native.current?.value ?? defaultValue ?? "");
  const update = (next: string) => emitInputValue(native.current, next, onChange, onValueChange);
  return <span className="ui-text-control">
    <input {...props} ref={native} type={secure ? "password" : "text"} value={value} defaultValue={defaultValue} onChange={(event) => { onValueChange?.(event.target.value); onChange?.(event); }} disabled={disabled} readOnly={readOnly} className={`ui-input ${className}`.trim()}/>
    <Button size="compact" iconOnly className="ui-input-keyboard" aria-label="Open keyboard" disabled={disabled || readOnly} onClick={() => setOpen(true)}><span className="ui-keyboard-icon" aria-hidden="true">⌨</span></Button>
    {clearable && current && <Button size="compact" iconOnly className="ui-input-clear" aria-label="Clear input" disabled={disabled || readOnly} onClick={() => { update(""); native.current?.focus(); }}>×</Button>}
    {open && (
      <InputModal kind="text" value={current} secure={secure} label={keyboardLabel ?? props["aria-label"]} onCommit={(next) => { update(next); setOpen(false); onKeyboardCommit?.(next); requestAnimationFrame(() => native.current?.focus()); }} onCancel={() => { setOpen(false); requestAnimationFrame(() => native.current?.focus()); }}/>
    )}
  </span>;
});

export interface NumberInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  allowDecimal?: boolean;
  showStepButtons?: boolean;
  keyboardLabel?: string;
  onValueChange?: (value: string) => void;
  unit?: ReactNode;
}

function clampNumber(value: number, min: NumberInputProps["min"], max: NumberInputProps["max"]) {
  return Math.max(min == null ? -Infinity : Number(min), Math.min(max == null ? Infinity : Number(max), value));
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput(
  { className = "", value, defaultValue, onChange, onValueChange, allowDecimal = false, showStepButtons = true, keyboardLabel, unit, disabled, readOnly, min, max, step = 1, ...props },
  ref,
) {
  const [open, setOpen] = useState(false);
  const native = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => native.current!);
  const current = String(value ?? native.current?.value ?? defaultValue ?? "");
  const normalize = (next: string) => {
    const filtered = allowDecimal ? next.replace(/[^\d.-]/g, "") : next.replace(/[^\d-]/g, "");
    if (filtered === "" || filtered === "-") return filtered;
    if (allowDecimal ? /^-?\d*\.?\d*$/.test(filtered) : /^-?\d+$/.test(filtered)) return filtered;
    return current;
  };
  const update = (next: string) => emitInputValue(native.current, normalize(next), onChange, onValueChange);
  const bump = (direction: -1 | 1) => update(String(clampNumber((Number(current) || 0) + Number(step) * direction, min, max)));
  const commit = (next: string) => {
    const parsed = Number(next);
    update(next === "" || next === "-" || Number.isNaN(parsed) ? "" : String(clampNumber(parsed, min, max)));
  };
  return <span className={`ui-number-control ${showStepButtons ? "with-steppers" : "without-steppers"}`}>
    {showStepButtons && <Button size="compact" iconOnly className="ui-number-minus" aria-label="Decrease value" disabled={disabled || readOnly} onClick={() => bump(-1)}><span className="ui-step-icon" aria-hidden="true">−</span></Button>}
    <input {...props} ref={native} type="text" inputMode={allowDecimal ? "decimal" : "numeric"} value={value} defaultValue={defaultValue} onChange={(event) => { const next = normalize(event.target.value); if (next !== event.target.value) event.target.value = next; onValueChange?.(next); onChange?.(event); }} onBlur={(event) => { commit(event.target.value); props.onBlur?.(event); }} disabled={disabled} readOnly={readOnly} className={`ui-input ${className}`.trim()}/>
    {showStepButtons && <Button size="compact" iconOnly className="ui-number-plus" aria-label="Increase value" disabled={disabled || readOnly} onClick={() => bump(1)}><span className="ui-step-icon" aria-hidden="true">+</span></Button>}
    <Button size="compact" iconOnly className="ui-input-keyboard" aria-label="Open number pad" disabled={disabled || readOnly} onClick={() => setOpen(true)}><span className="ui-keyboard-icon" aria-hidden="true">⌨</span></Button>
    {open && (
      <InputModal kind="number" value={current} allowDecimal={allowDecimal} label={keyboardLabel ?? props["aria-label"]} unit={unit} onCommit={(next) => { commit(next); setOpen(false); requestAnimationFrame(() => native.current?.focus()); }} onCancel={() => { setOpen(false); requestAnimationFrame(() => native.current?.focus()); }}/>
    )}
  </span>;
});

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  allowDecimal?: boolean;
  showStepButtons?: boolean;
}

const WheelSafeRangeInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function WheelSafeRangeInput({ className = "", onWheel: _onWheel, ...props }, ref) {
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
  return <input {...props} ref={native} type="range" className={`ui-native-control ${className}`.trim()}/>;
});

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input({ className = "", type = "text", allowDecimal, showStepButtons, ...props }, ref) {
  if (type === "number") return <NumberInput {...props} allowDecimal={allowDecimal} showStepButtons={showStepButtons} className={className} ref={ref}/>;
  if (type === "range") return <WheelSafeRangeInput {...props} className={className} ref={ref}/>;
  const native = ["checkbox", "radio", "range", "file", "color", "hidden"].includes(type);
  return <input ref={ref} type={type} className={`${native ? "ui-native-control" : "ui-input"} ${className}`.trim()} {...props}/>;
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function TextArea({ className = "", ...props }, ref) {
  return <textarea ref={ref} className={`ui-textarea ${className}`.trim()} {...props}/>;
});

type TextFieldProps = TextInputProps & { label?: ReactNode; description?: ReactNode; error?: ReactNode; controlSize?: ControlSize; labelPlacement?: LabelPlacement };
export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField({ label, description, error, controlSize = "default", id, className = "", required, labelPlacement, ...props }, ref) {
  const generated = useId();
  const fieldId = id ?? generated;
  return <FormField label={label} description={description} error={error} required={required} htmlFor={fieldId} labelPlacement={labelPlacement}><TextInput {...props} required={required} ref={ref} id={fieldId} aria-invalid={Boolean(error) || undefined} className={`ui-${controlSize} ${className}`.trim()} keyboardLabel={props.keyboardLabel ?? (typeof label === "string" ? label : undefined)}/></FormField>;
});

type NumberFieldProps = NumberInputProps & { label?: ReactNode; description?: ReactNode; error?: ReactNode; labelPlacement?: LabelPlacement };
export const NumberField = forwardRef<HTMLInputElement, NumberFieldProps>(function NumberField({ label, description, error, id, required, className = "", labelPlacement, ...props }, ref) {
  const generated = useId();
  const fieldId = id ?? generated;
  return <FormField label={label} description={description} error={error} required={required} htmlFor={fieldId} labelPlacement={labelPlacement}><NumberInput {...props} required={required} id={fieldId} className={className} keyboardLabel={props.keyboardLabel ?? (typeof label === "string" ? label : undefined)} ref={ref}/></FormField>;
});

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement> & { label?: ReactNode; description?: ReactNode; error?: ReactNode; labelPlacement?: LabelPlacement }>(function TextAreaField({ label, description, error, id, className = "", required, labelPlacement, ...props }, ref) {
  const generated = useId();
  const fieldId = id ?? generated;
  return <FormField label={label} description={description} error={error} required={required} htmlFor={fieldId} labelPlacement={labelPlacement}><textarea {...props} required={required} ref={ref} id={fieldId} aria-invalid={Boolean(error) || undefined} className={`ui-textarea ${className}`.trim()}/></FormField>;
});

export interface SelectOption<T extends string = string> { value: T; label: ReactNode; disabled?: boolean }

export interface MultiValueToggleProps<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
}

export function MultiValueToggle<T extends string>({ value, options, onChange, disabled = false, ariaLabel = "Options", className = "" }: MultiValueToggleProps<T>) {
  return <div className={`ui-multi-value-toggle ${className}`.trim()} role="radiogroup" aria-label={ariaLabel}>
    {options.map((option) => <Button
      role="radio"
      aria-checked={option.value === value}
      active={option.value === value}
      disabled={disabled || option.disabled}
      key={option.value}
      onClick={() => onChange(option.value)}
    >{option.label}</Button>)}
  </div>;
}

export function MultiValueToggleField<T extends string>({ label, description, error, labelPlacement, ...props }: MultiValueToggleProps<T> & {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  labelPlacement?: LabelPlacement;
}) {
  return <FormField label={label} description={description} error={error} labelPlacement={labelPlacement}>
    <MultiValueToggle {...props} ariaLabel={props.ariaLabel ?? (typeof label === "string" ? label : undefined)}/>
  </FormField>;
}

export function SelectField<T extends string>({ label, value, options, onChange, description, error, disabled, size = "default", className = "", labelPlacement }: {
  label?: ReactNode;
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  description?: ReactNode;
  error?: ReactNode;
  disabled?: boolean;
  size?: ControlSize;
  className?: string;
  labelPlacement?: LabelPlacement;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(() => Math.max(0, options.findIndex((item) => item.value === value)));
  const [position, setPosition] = useState<CSSProperties>({});
  const button = useRef<HTMLButtonElement>(null);
  const place = () => {
    const box = button.current?.getBoundingClientRect();
    const layoutWidth = button.current?.offsetWidth;
    const layoutHeight = button.current?.offsetHeight;
    if (!box || !layoutWidth || !layoutHeight) return;
    const left = box.left - (layoutWidth - box.width) / 2;
    const top = box.top - (layoutHeight - box.height) / 2;
    const bottom = top + layoutHeight;
    const popupWidth = Math.max(layoutWidth, 240);
    const below = window.innerHeight - bottom;
    const maxHeight = Math.max(160, Math.min(440, below > 220 ? below - 8 : box.top - 8));
    setPosition({ left: Math.max(8, Math.min(left, window.innerWidth - popupWidth - 8)), top: below > 220 ? bottom + 4 : undefined, bottom: below <= 220 ? window.innerHeight - top + 4 : undefined, width: popupWidth, maxHeight });
  };
  useEffect(() => {
    if (!open) return;
    place();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); setOpen(false); button.current?.focus(); }
      if (event.key === "ArrowDown") { event.preventDefault(); setActive((index) => Math.min(options.length - 1, index + 1)); }
      if (event.key === "ArrowUp") { event.preventDefault(); setActive((index) => Math.max(0, index - 1)); }
      if (event.key === "Enter" && options[active] && !options[active].disabled) { event.preventDefault(); onChange(options[active].value); setOpen(false); button.current?.focus(); }
    };
    window.addEventListener("keydown", keydown, true);
    window.addEventListener("resize", place);
    return () => { window.removeEventListener("keydown", keydown, true); window.removeEventListener("resize", place); };
  }, [open, active, options, onChange]);
  const chosen = options.find((item) => item.value === value);
  return <FormField label={label} description={description} error={error} className={className} labelPlacement={labelPlacement}>
    <Button ref={button} className="ui-select-trigger" size={size} disabled={disabled} aria-haspopup="listbox" aria-expanded={open} onClick={() => { if (!open) place(); setOpen(!open); }}><span>{chosen?.label ?? value}</span><i aria-hidden="true">▼</i></Button>
    {open && createPortal(<div className="ui-select-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) { setOpen(false); button.current?.focus(); } }}><div className="ui-select-options" style={position} role="listbox" aria-label={typeof label === "string" ? label : "Options"}>{options.map((option, index) => <Button role="option" aria-selected={option.value === value} active={option.value === value} className={index === active ? "is-highlighted" : ""} disabled={option.disabled} key={option.value} onPointerMove={() => setActive(index)} onClick={() => { onChange(option.value); setOpen(false); button.current?.focus(); }}>{option.label}</Button>)}</div></div>, document.body)}
  </FormField>;
}

export const Select = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(function Select({ className = "", children, value, defaultValue, onChange, disabled, "aria-label": ariaLabel, ...props }, ref) {
  const native = useRef<HTMLSelectElement>(null);
  useImperativeHandle(ref, () => native.current!);
  const options = Children.toArray(children).filter(isValidElement).map((child) => {
    const option = child.props as { value?: string | number; children?: ReactNode; disabled?: boolean };
    const label = option.children;
    return { value: String(option.value ?? (typeof label === "string" || typeof label === "number" ? label : "")), label, disabled: option.disabled };
  });
  const selected = String(value ?? defaultValue ?? options[0]?.value ?? "");
  return <><select {...props} ref={native} value={selected} onChange={onChange} disabled={disabled} className="ui-visually-hidden-select" aria-hidden="true" tabIndex={-1}>{children}</select><SelectField label={ariaLabel} className={className} value={selected} disabled={disabled} options={options} onChange={(next) => { if (native.current) native.current.value = next; onChange?.({ target: native.current, currentTarget: native.current } as ChangeEvent<HTMLSelectElement>); }}/></>;
});

type CheckProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "size"> & { label: ReactNode; description?: ReactNode; error?: ReactNode; labelPlacement?: LabelPlacement };
export const CheckboxField = forwardRef<HTMLInputElement, CheckProps>(function CheckboxField({ label, description, error, labelPlacement, className = "", id, "aria-label": ariaLabel, ...props }, ref) {
  const generated = useId();
  const fieldId = id ?? generated;
  return <FormField label={label} description={description} error={error} htmlFor={fieldId} labelPlacement={labelPlacement} className={className}>
    <label className="ui-check-control"><input {...props} ref={ref} id={fieldId} aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)} type="checkbox"/><span className="ui-check-mark" aria-hidden="true">✓</span><span className="ui-check-state">{props.checked ? "Checked" : "Unchecked"}</span></label>
  </FormField>;
});
export const SwitchField = forwardRef<HTMLInputElement, CheckProps>(function SwitchField({ label, description, error, labelPlacement, className = "", id, "aria-label": ariaLabel, ...props }, ref) {
  const generated = useId();
  const fieldId = id ?? generated;
  return <FormField label={label} description={description} error={error} htmlFor={fieldId} labelPlacement={labelPlacement} className={className}>
    <label className="ui-switch-control"><input {...props} ref={ref} id={fieldId} aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)} type="checkbox" role="switch"/><span className="ui-switch-track" aria-hidden="true"><i/></span><span className="ui-check-state">{props.checked ? "On" : "Off"}</span></label>
  </FormField>;
});

export const DEFAULT_ICONS = ["⊞", "⌂", "★", "◉", "▶", "▣", "⚙", "◇", "◆", "●", "○", "✦", "☀", "◐", "▰", "⌖"];
export const DEFAULT_COLORS = ["#d98236", "#f0b84b", "#62d78a", "#1bd6ec", "#5c9dff", "#d76cff", "#f275d9", "#e0555f", "#ffffff", "#aeb8bf", "#56636d", "#20262b"];

function PickerDialog({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return createPortal(<div className="stacked-modal-layer ui-picker-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}><section className="nested-modal ui-picker-dialog" role="dialog" aria-modal="true" aria-label={title}><ModalTitleBar title={title} closeLabel={`Close ${title}`} onClose={onClose}/>{children}</section></div>, document.body);
}

export function IconPickerField({ label, value, onChange, icons = DEFAULT_ICONS, description, disabled, labelPlacement }: { label?: ReactNode; value: string; onChange: (value: string) => void; icons?: string[]; description?: ReactNode; disabled?: boolean; labelPlacement?: LabelPlacement }) {
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState(value);
  return <FormField label={label} description={description} labelPlacement={labelPlacement}><Button className="ui-picker-trigger" disabled={disabled} aria-haspopup="dialog" onClick={() => { setCustom(value); setOpen(true); }}><span className="ui-picker-preview">{value || "◇"}</span><span>Choose icon</span></Button>{open && <PickerDialog title="Choose icon" onClose={() => setOpen(false)}><div className="ui-icon-grid">{icons.map((icon) => <Button key={icon} active={icon === value} aria-label={`Use ${icon}`} onClick={() => { onChange(icon); setOpen(false); }}>{icon}</Button>)}</div><FormLayout labelPlacement="side"><TextField label="Custom" value={custom} maxLength={4} clearable onChange={(event) => setCustom(event.target.value)}/><FormField label=""><Button variant="primary" disabled={!custom.trim()} onClick={() => { onChange(custom.trim()); setOpen(false); }}>Use custom icon</Button></FormField></FormLayout></PickerDialog>}</FormField>;
}

function validHex(value: string) { return /^#[0-9a-f]{6}$/i.test(value); }
export function ColorPickerField({ label, value, onChange, colors = DEFAULT_COLORS, description, disabled, labelPlacement }: { label?: ReactNode; value: string; onChange: (value: string) => void; colors?: string[]; description?: ReactNode; disabled?: boolean; labelPlacement?: LabelPlacement }) {
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState(value);
  const normalized = validHex(value) ? value : "#d98236";
  return <FormField label={label} description={description} labelPlacement={labelPlacement}><Button className="ui-picker-trigger" disabled={disabled} aria-haspopup="dialog" onClick={() => { setCustom(normalized); setOpen(true); }}><span className="ui-color-preview" style={{ background: normalized }}/><span>{normalized.toUpperCase()}</span></Button>{open && <PickerDialog title="Choose color" onClose={() => setOpen(false)}><div className="ui-color-grid">{colors.map((color) => <Button key={color} active={color.toLowerCase() === normalized.toLowerCase()} aria-label={`Use color ${color}`} style={{ "--picker-color": color } as CSSProperties} onClick={() => { onChange(color); setOpen(false); }}><span style={{ background: color }}/></Button>)}</div><FormLayout labelPlacement="side"><TextField label="Custom hex" value={custom} clearable onChange={(event) => setCustom(event.target.value)}/><FormField label="Preview"><span className="ui-custom-color-preview" style={{ background: validHex(custom) ? custom : "transparent" }}/></FormField><FormField label=""><Button variant="primary" disabled={!validHex(custom)} onClick={() => { onChange(custom.toLowerCase()); setOpen(false); }}>Use custom color</Button></FormField></FormLayout></PickerDialog>}</FormField>;
}
