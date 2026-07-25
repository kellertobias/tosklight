import {
  Children,
  forwardRef,
  isValidElement,
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type ForwardedRef,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import {
  Button,
  FormField,
  type ControlSize,
  type LabelPlacement,
} from "./foundation";

export interface SelectOption<T extends string = string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface MultiValueToggleProps<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
}

export function MultiValueToggle<T extends string>({
  value, options, onChange, disabled = false, ariaLabel = "Options", className = "",
}: MultiValueToggleProps<T>) {
  return <div className={`ui-multi-value-toggle ${className}`.trim()}
    role="radiogroup" aria-label={ariaLabel}>
    {options.map((option) => <Button role="radio" key={option.value}
      aria-checked={option.value === value} active={option.value === value}
      disabled={disabled || option.disabled} onClick={() => onChange(option.value)}>
      {option.label}
    </Button>)}
  </div>;
}

type MultiValueToggleFieldProps<T extends string> = MultiValueToggleProps<T> & {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  labelPlacement?: LabelPlacement;
};

export function MultiValueToggleField<T extends string>({
  label, description, error, labelPlacement, ...props
}: MultiValueToggleFieldProps<T>) {
  const ariaLabel = props.ariaLabel ?? (typeof label === "string" ? label : undefined);
  return <FormField label={label} description={description} error={error}
    labelPlacement={labelPlacement}>
    <MultiValueToggle {...props} ariaLabel={ariaLabel}/>
  </FormField>;
}

interface SelectFieldProps<T extends string> {
  label?: ReactNode;
  ariaLabel?: string;
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  description?: ReactNode;
  error?: ReactNode;
  disabled?: boolean;
  size?: ControlSize;
  className?: string;
  labelPlacement?: LabelPlacement;
}

function popupPosition(button: HTMLButtonElement | null): CSSProperties | undefined {
  const box = button?.getBoundingClientRect();
  const layoutWidth = button?.offsetWidth;
  const layoutHeight = button?.offsetHeight;
  if (!box || !layoutWidth || !layoutHeight) return;
  const left = box.left - (layoutWidth - box.width) / 2;
  const top = box.top - (layoutHeight - box.height) / 2;
  const bottom = top + layoutHeight;
  const popupWidth = Math.max(layoutWidth, 240);
  const below = window.innerHeight - bottom;
  const maxHeight = Math.max(160, Math.min(440, below > 220 ? below - 8 : box.top - 8));
  return {
    left: Math.max(8, Math.min(left, window.innerWidth - popupWidth - 8)),
    top: below > 220 ? bottom + 4 : undefined,
    bottom: below <= 220 ? window.innerHeight - top + 4 : undefined,
    width: popupWidth,
    maxHeight,
  };
}

interface SelectKeyboardController<T extends string> {
  active: number;
  options: SelectOption<T>[];
  setActive: (update: (index: number) => number) => void;
  choose: (value: T) => void;
  close: () => void;
}

function handleSelectKey<T extends string>(
  event: KeyboardEvent,
  controller: SelectKeyboardController<T>,
) {
  if (event.key === "Escape") controller.close();
  else if (event.key === "ArrowDown") {
    controller.setActive((index) => Math.min(controller.options.length - 1, index + 1));
  } else if (event.key === "ArrowUp") {
    controller.setActive((index) => Math.max(0, index - 1));
  } else if (event.key === "Enter") {
    const option = controller.options[controller.active];
    if (option && !option.disabled) controller.choose(option.value);
    else return;
  } else return;
  event.preventDefault();
}

function SelectOptions<T extends string>({
  label, options, value, active, position, setActive, choose, close,
}: {
  label?: ReactNode;
  options: SelectOption<T>[];
  value: T;
  active: number;
  position: CSSProperties;
  setActive: (index: number) => void;
  choose: (value: T) => void;
  close: () => void;
}) {
  return createPortal(<div className="ui-select-backdrop"
    onPointerDown={(event) => event.target === event.currentTarget && close()}>
    <div className="ui-select-options" style={position} role="listbox"
      aria-label={typeof label === "string" ? label : "Options"}>
      {options.map((option, index) => <Button role="option" key={option.value}
        aria-selected={option.value === value} active={option.value === value}
        className={index === active ? "is-highlighted" : ""} disabled={option.disabled}
        onPointerMove={() => setActive(index)} onClick={() => choose(option.value)}>
        {option.label}
      </Button>)}
    </div>
  </div>, document.body);
}

export function SelectField<T extends string>({
  label, value, options, onChange, description, error, disabled, size = "default",
  className = "", labelPlacement, ariaLabel,
}: SelectFieldProps<T>) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(() => Math.max(0, options.findIndex((item) => item.value === value)));
  const [position, setPosition] = useState<CSSProperties>({});
  const button = useRef<HTMLButtonElement>(null);
  const place = () => setPosition(popupPosition(button.current) ?? {});
  const close = () => { setOpen(false); button.current?.focus(); };
  const choose = (next: T) => { onChange(next); close(); };
  useEffect(() => {
    if (!open) return;
    place();
    const keydown = (event: KeyboardEvent) => handleSelectKey(event, {
      active, options, setActive, choose, close,
    });
    window.addEventListener("keydown", keydown, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("keydown", keydown, true);
      window.removeEventListener("resize", place);
    };
  }, [open, active, options, onChange]);
  const chosen = options.find((item) => item.value === value);
  return <FormField label={label} description={description} error={error}
    className={className} labelPlacement={labelPlacement}>
    <Button ref={button} className="ui-select-trigger" size={size} disabled={disabled}
      aria-label={ariaLabel}
      aria-haspopup="listbox" aria-expanded={open}
      onClick={() => { if (!open) place(); setOpen(!open); }}>
      <span>{chosen?.label ?? value}</span><i aria-hidden="true">▼</i>
    </Button>
    {open && <SelectOptions label={label} options={options} value={value} active={active}
      position={position} setActive={setActive} choose={choose} close={close}/>}
  </FormField>;
}

export const Select = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(
  function Select({
    className = "", children, value, defaultValue, onChange, disabled,
    "aria-label": ariaLabel, ...props
  }, ref) {
    const native = useRef<HTMLSelectElement>(null);
    useImperativeHandle(ref, () => native.current!);
    const options = Children.toArray(children).filter(isValidElement).map((child) => {
      const option = child.props as {
        value?: string | number; children?: ReactNode; disabled?: boolean;
      };
      const label = option.children;
      return {
        value: String(option.value ?? (typeof label === "string" || typeof label === "number" ? label : "")),
        label,
        disabled: option.disabled,
      };
    });
    const selected = String(value ?? defaultValue ?? options[0]?.value ?? "");
    const select = (next: string) => {
      if (native.current) native.current.value = next;
      onChange?.({ target: native.current, currentTarget: native.current } as ChangeEvent<HTMLSelectElement>);
    };
    return <>
      <select {...props} ref={native} value={selected} onChange={onChange} disabled={disabled}
        className="ui-visually-hidden-select" aria-hidden="true" tabIndex={-1}>{children}</select>
      <SelectField label={ariaLabel} className={className} value={selected}
        disabled={disabled} options={options} onChange={select}/>
    </>;
  },
);

type CheckProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "size"> & {
  label: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  labelPlacement?: LabelPlacement;
};

function useFieldId(id?: string) {
  const generated = useId();
  return id ?? generated;
}

function CheckState({ checked, kind }: { checked?: boolean; kind: "check" | "switch" }) {
  if (kind === "switch") {
    return <><span className="ui-switch-track" aria-hidden="true"><i/></span>
      <span className="ui-check-state">{checked ? "On" : "Off"}</span></>;
  }
  return <><span className="ui-check-mark" aria-hidden="true">✓</span>
    <span className="ui-check-state">{checked ? "Checked" : "Unchecked"}</span></>;
}

function renderCheckField(
  kind: "check" | "switch",
  { label, description, error, labelPlacement, className = "", id, "aria-label": ariaLabel, ...props }: CheckProps,
  ref: ForwardedRef<HTMLInputElement>,
  fieldId: string,
) {
  return <FormField label={label} description={description} error={error}
    htmlFor={fieldId} labelPlacement={labelPlacement} className={className}>
    <label className={kind === "switch" ? "ui-switch-control" : "ui-check-control"}>
      <input {...props} ref={ref} id={fieldId}
        aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)}
        type="checkbox" role={kind === "switch" ? "switch" : undefined}/>
      <CheckState checked={props.checked} kind={kind}/>
    </label>
  </FormField>;
}

export const CheckboxField = forwardRef<HTMLInputElement, CheckProps>(function CheckboxField(
  props, ref,
) {
  return renderCheckField("check", props, ref, useFieldId(props.id));
});

export const SwitchField = forwardRef<HTMLInputElement, CheckProps>(function SwitchField(
  props, ref,
) {
  return renderCheckField("switch", props, ref, useFieldId(props.id));
});
