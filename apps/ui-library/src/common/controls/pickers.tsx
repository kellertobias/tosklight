import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { ModalLayer } from "../../modals/ModalStack";
import { activeModalPortalRoot } from "../ModalPortal";
import { ModalTitleBar } from "../ModalTitleBar";
import { TextField } from "./formFields";
import {
  Button,
  FormField,
  type LabelPlacement,
} from "./foundation";
import {
  ICON_CATALOG_GROUPS,
  iconCatalogItem,
  resolveIconGroup,
  type IconCatalogGroup,
} from "./iconCatalog";

export const DEFAULT_COLORS = [
  "#ef4444", "#f97316", "#f59e0b", "#eab308",
  "#84cc16", "#22c55e", "#20c997", "#06b6d4",
  "#0ea5e9", "#3b82f6", "#6366f1", "#8b5cf6",
  "#a855f7", "#ec4899", "#f43f5e", "#f8fafc",
] as const;

interface PickerFieldProps {
  label?: ReactNode;
  value: string;
  onChange: (value: string) => void;
  description?: ReactNode;
  disabled?: boolean;
  labelPlacement?: LabelPlacement;
}

function PickerDialog({
  title, actions, children, onClose,
}: { title: string; actions?: ReactNode; children: ReactNode; onClose: () => void }) {
  return <ModalLayer
    ariaLabel={title}
    className="ui-picker-layer"
    dialogClassName="ui-picker-dialog"
    onClose={onClose}
  >
    <ModalTitleBar title={title} actions={actions} closeLabel={`Close ${title}`} onClose={onClose}/>
    {children}
  </ModalLayer>;
}

type IconPickerFieldProps = PickerFieldProps & {
  defaultGroup?: string;
  groups?: readonly IconCatalogGroup[];
};

export function IconPickerField({
  label, value, onChange, defaultGroup, groups = ICON_CATALOG_GROUPS,
  description, disabled, labelPlacement,
}: IconPickerFieldProps) {
  const [open, setOpen] = useState(false);
  const fallback = groups.some((group) => group.id === defaultGroup)
    ? defaultGroup!
    : groups === ICON_CATALOG_GROUPS
      ? resolveIconGroup(defaultGroup)
      : groups[0]?.id ?? "";
  const [groupId, setGroupId] = useState(fallback);
  const group = groups.find((candidate) => candidate.id === groupId) ?? groups[0];
  const current = iconCatalogItem(value);
  const choose = (next: string) => { onChange(next); setOpen(false); };
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button className="ui-picker-trigger" disabled={disabled} aria-haspopup="dialog"
      aria-label="Choose icon"
      onClick={() => { setGroupId(fallback); setOpen(true); }}>
      <span className="ui-picker-preview">{current?.source === "catalog"
        ? <img src={current.value} alt=""/>
        : value || "◇"}</span>
      <span>{current?.label ?? "Choose icon"}</span>
    </Button>
    {open && <PickerDialog title="Choose icon" onClose={() => setOpen(false)}
      actions={<label className="ui-icon-group-control">
        <span>Icon group</span>
        <select aria-label="Icon group" value={group?.id ?? ""}
          onChange={(event) => setGroupId(event.target.value)}>
          {groups.map((candidate) => <option key={candidate.id} value={candidate.id}>
            {candidate.label}
          </option>)}
        </select>
      </label>}>
      <div className="ui-icon-grid" data-icon-group={group?.id}>
        {group?.icons.map((icon) => <Button key={icon.value}
          active={icon.value === value}
          aria-label={icon.source === "built-in" ? `Use ${icon.value}` : icon.label}
          title={icon.label} onClick={() => choose(icon.value)}>
          {icon.source === "catalog" ? <img src={icon.value} alt=""/> : icon.value}
        </Button>)}
      </div>
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
  colors: readonly string[];
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
  </div>, activeModalPortalRoot());
}

type ColorPickerFieldProps = PickerFieldProps & { colors?: readonly string[] };

export function ColorPickerField({
  label, value, onChange, colors = DEFAULT_COLORS, description, disabled, labelPlacement,
}: ColorPickerFieldProps) {
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState(value);
  const [position, setPosition] = useState<CSSProperties>({});
  const button = useRef<HTMLButtonElement>(null);
  const normalized = validHex(value) ? value : DEFAULT_COLORS[0];
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
  const style = { "--picker-color": normalized } as CSSProperties;
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button ref={button} className="ui-color-input-trigger" disabled={disabled}
      aria-haspopup="listbox" aria-expanded={open} style={style}
      onClick={() => { if (!open) { setCustom(normalized); place(); } setOpen(!open); }}>
      <span className="ui-color-trigger-swatch" style={{ color: colorInputText(normalized) }}>
        <b>{normalized.toUpperCase()}</b>
      </span>
      <i aria-hidden="true">▼</i>
    </Button>
    {open && <ColorPopup label={label} colors={colors} normalized={normalized}
      custom={custom} position={position} setCustom={setCustom} choose={choose} close={close}/>}
  </FormField>;
}
