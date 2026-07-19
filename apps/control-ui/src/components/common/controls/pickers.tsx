import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { ModalTitleBar } from "../ModalTitleBar";
import { TextField } from "./formFields";
import {
  Button,
  FormField,
  FormLayout,
  type LabelPlacement,
} from "./foundation";

export const DEFAULT_ICONS = [
  "⊞", "⌂", "★", "◉", "▶", "▣", "⚙", "◇", "◆", "●", "○", "✦", "☀", "◐", "▰", "⌖",
];

export const DEFAULT_COLORS = [
  "#d98236", "#f0b84b", "#62d78a", "#1bd6ec", "#5c9dff", "#d76cff",
  "#f275d9", "#e0555f", "#ffffff", "#aeb8bf", "#56636d", "#20262b",
];

interface PickerFieldProps {
  label?: ReactNode;
  value: string;
  onChange: (value: string) => void;
  description?: ReactNode;
  disabled?: boolean;
  labelPlacement?: LabelPlacement;
}

function PickerDialog({
  title, children, onClose,
}: { title: string; children: ReactNode; onClose: () => void }) {
  return createPortal(<div className="stacked-modal-layer ui-picker-layer"
    onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal ui-picker-dialog" role="dialog"
      aria-modal="true" aria-label={title}>
      <ModalTitleBar title={title} closeLabel={`Close ${title}`} onClose={onClose}/>
      {children}
    </section>
  </div>, document.body);
}

type IconPickerFieldProps = PickerFieldProps & { icons?: string[] };

export function IconPickerField({
  label, value, onChange, icons = DEFAULT_ICONS, description, disabled, labelPlacement,
}: IconPickerFieldProps) {
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState(value);
  const choose = (next: string) => { onChange(next); setOpen(false); };
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button className="ui-picker-trigger" disabled={disabled} aria-haspopup="dialog"
      onClick={() => { setCustom(value); setOpen(true); }}>
      <span className="ui-picker-preview">{value || "◇"}</span><span>Choose icon</span>
    </Button>
    {open && <PickerDialog title="Choose icon" onClose={() => setOpen(false)}>
      <div className="ui-icon-grid">{icons.map((icon) => <Button key={icon}
        active={icon === value} aria-label={`Use ${icon}`} onClick={() => choose(icon)}>
        {icon}
      </Button>)}</div>
      <FormLayout labelPlacement="side">
        <TextField label="Custom" value={custom} maxLength={4} clearable
          onChange={(event) => setCustom(event.target.value)}/>
        <FormField label=""><Button variant="primary" disabled={!custom.trim()}
          onClick={() => choose(custom.trim())}>Use custom icon</Button></FormField>
      </FormLayout>
    </PickerDialog>}
  </FormField>;
}

function validHex(value: string) {
  return /^#[0-9a-f]{6}$/i.test(value);
}

function colorInputText(color: string) {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(color);
  if (!match) return "#fff";
  const luminance = Number.parseInt(match[1], 16) * .299
    + Number.parseInt(match[2], 16) * .587
    + Number.parseInt(match[3], 16) * .114;
  return luminance > 155 ? "#101419" : "#fff";
}

function colorPopupPosition(button: HTMLButtonElement | null): CSSProperties | undefined {
  const box = button?.getBoundingClientRect();
  const layoutWidth = button?.offsetWidth;
  const layoutHeight = button?.offsetHeight;
  if (!box || !layoutWidth || !layoutHeight) return;
  const left = box.left - (layoutWidth - box.width) / 2;
  const top = box.top - (layoutHeight - box.height) / 2;
  const bottom = top + layoutHeight;
  const popupWidth = Math.max(layoutWidth, Math.min(420, window.innerWidth - 16));
  const below = window.innerHeight - bottom;
  const maxHeight = Math.max(180, Math.min(470, below > 260 ? below - 8 : top - 8));
  return {
    left: Math.max(8, Math.min(left, window.innerWidth - popupWidth - 8)),
    top: below > 260 ? bottom + 4 : undefined,
    bottom: below <= 260 ? window.innerHeight - top + 4 : undefined,
    width: popupWidth,
    maxHeight,
  };
}

function ColorPopup({
  label, colors, normalized, custom, position, setCustom, choose, close,
}: {
  label?: ReactNode;
  colors: string[];
  normalized: string;
  custom: string;
  position: CSSProperties;
  setCustom: (value: string) => void;
  choose: (value: string) => void;
  close: () => void;
}) {
  const ariaLabel = typeof label === "string" ? label : "Color picker";
  return createPortal(<div className="ui-color-dropdown-backdrop"
    onPointerDown={(event) => event.target === event.currentTarget && close()}>
    <section className="ui-color-dropdown-panel touch-scrollbars" style={position}
      aria-label={ariaLabel}>
      <div className="ui-color-dropdown-grid" role="listbox"
        aria-label={typeof label === "string" ? label : "Colors"}>
        {colors.map((color) => <Button role="option" key={color}
          aria-selected={color.toLowerCase() === normalized.toLowerCase()}
          active={color.toLowerCase() === normalized.toLowerCase()}
          aria-label={`Use color ${color}`} style={{ "--picker-color": color } as CSSProperties}
          onClick={() => choose(color)}><span style={{ background: color }}/></Button>)}
      </div>
      <div className="ui-color-dropdown-custom">
        <TextField label="Custom hex" value={custom} clearable
          onChange={(event) => setCustom(event.target.value)}/>
        <span className="ui-custom-color-preview" aria-label="Color preview"
          style={{ background: validHex(custom) ? custom : "transparent" }}/>
        <Button variant="primary" disabled={!validHex(custom)}
          onClick={() => choose(custom)}>Use custom color</Button>
      </div>
    </section>
  </div>, document.body);
}

type ColorPickerFieldProps = PickerFieldProps & { colors?: string[] };

export function ColorPickerField({
  label, value, onChange, colors = DEFAULT_COLORS, description, disabled, labelPlacement,
}: ColorPickerFieldProps) {
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState(value);
  const [position, setPosition] = useState<CSSProperties>({});
  const button = useRef<HTMLButtonElement>(null);
  const normalized = validHex(value) ? value : "#d98236";
  const place = () => setPosition(colorPopupPosition(button.current) ?? {});
  const close = () => { setOpen(false); button.current?.focus(); };
  const choose = (color: string) => { onChange(color.toLowerCase()); close(); };
  useEffect(() => {
    if (!open) return;
    place();
    const escape = (event: KeyboardEvent) => event.key === "Escape" && close();
    window.addEventListener("keydown", escape, true);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("keydown", escape, true);
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open]);
  const style = { "--picker-color": normalized, color: colorInputText(normalized) } as CSSProperties;
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button ref={button} className="ui-color-input-trigger" disabled={disabled}
      aria-haspopup="listbox" aria-expanded={open} style={style}
      onClick={() => { if (!open) { setCustom(normalized); place(); } setOpen(!open); }}>
      <span>{normalized.toUpperCase()}</span><i aria-hidden="true">▼</i>
    </Button>
    {open && <ColorPopup label={label} colors={colors} normalized={normalized}
      custom={custom} position={position} setCustom={setCustom} choose={choose} close={close}/>}
  </FormField>;
}
