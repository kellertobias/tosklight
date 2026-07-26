import { useState, type ReactNode } from "react";
import { ModalLayer } from "../../modals/ModalStack";
import { ModalTitleBar } from "../ModalTitleBar";
import { Button, FormField, type LabelPlacement } from "./foundation";

export interface GroupedSelectionOption<T extends string> {
  value: T;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}

export interface GroupedSelectionGroup<T extends string> {
  label: string;
  options: readonly GroupedSelectionOption<T>[];
}

export interface GroupedSelectionClearAction<T extends string> {
  label: string;
  value: T;
}

export interface GroupedSelectionFieldProps<T extends string> {
  label: ReactNode;
  dialogTitle?: string;
  closeLabel?: string;
  value: T;
  groups: readonly GroupedSelectionGroup<T>[];
  onChange: (value: T) => void;
  clearAction?: GroupedSelectionClearAction<T>;
  disabled?: boolean;
  description?: ReactNode;
  labelPlacement?: LabelPlacement;
}

export function GroupedSelectionField<T extends string>({
  label,
  dialogTitle,
  closeLabel,
  value,
  groups,
  onChange,
  clearAction,
  disabled = false,
  description,
  labelPlacement,
}: GroupedSelectionFieldProps<T>) {
  const [open, setOpen] = useState(false);
  const options = groups.flatMap((group) => group.options);
  const selected = options.find((option) => option.value === value);
  const title = dialogTitle ?? `Choose ${typeof label === "string" ? label : "option"}`;
  const clearSelected = clearAction?.value === value;
  const choose = (next: T) => {
    onChange(next);
    setOpen(false);
  };
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button
      className={`ui-select-trigger ui-grouped-selection-trigger ${clearSelected ? "is-empty" : ""}`}
      disabled={disabled}
      aria-haspopup="dialog"
      aria-expanded={open}
      onClick={() => setOpen(true)}
    >
      <span>{clearSelected ? clearAction.label : selected?.label ?? value}</span>
      <i aria-hidden="true">›</i>
    </Button>
    {open && (
      <ModalLayer
        ariaLabel={title}
        className="ui-grouped-selection-layer"
        dialogClassName="ui-grouped-selection-modal"
        onClose={() => setOpen(false)}
      >
        <ModalTitleBar
          title={title}
          actions={clearAction
            ? <Button variant="danger" onClick={() => choose(clearAction.value)}>
                {clearAction.label}
              </Button>
            : undefined}
          closeLabel={closeLabel ?? `Close ${title}`}
          onClose={() => setOpen(false)}
        />
        <div className="ui-grouped-selection-groups">
          {groups.map((group) => <section key={group.label}>
            <h3>{group.label}</h3>
            <div className="ui-grouped-selection-options">
              {group.options.map((option) => <Button
                key={option.value}
                active={option.value === value}
                aria-pressed={option.value === value}
                disabled={option.disabled}
                onClick={() => choose(option.value)}
              >
                <b>{option.label}</b>
                {option.description && <small>{option.description}</small>}
              </Button>)}
            </div>
          </section>)}
        </div>
      </ModalLayer>
    )}
  </FormField>;
}
