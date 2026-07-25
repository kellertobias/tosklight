import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ModalNumberInput, ModalTextKeyboard } from "../../input/ModalInputControls";
import { ModalTitleBar } from "../ModalTitleBar";

export interface InputModalProps {
  kind: "text" | "multiline" | "number";
  value: string;
  allowDecimal?: boolean;
  secure?: boolean;
  label?: string;
  unit?: ReactNode;
  onCommit: (value: string) => void;
  onDraftChange?: (value: string) => void;
  onCancel: () => void;
}

function ValuePreview({
  kind, label, secure, unit, value,
}: Pick<InputModalProps, "kind" | "label" | "secure" | "unit" | "value">) {
  if (kind === "multiline") {
    return <textarea className="ui-textarea modal-multiline-value"
      aria-label={`${label ?? "Text input"} value`} value={value} readOnly/>;
  }
  if (kind === "number" && unit) {
    return <div className="modal-number-value">
      <input className="ui-input" type="text" aria-label={`${label ?? kind} value`}
        value={value} readOnly/>
      <span aria-label="Unit">{unit}</span>
    </div>;
  }
  return <input className="ui-input" type={secure ? "password" : "text"}
    aria-label={`${label ?? kind} value`} value={value} readOnly/>;
}

export function InputModal({
  kind, value, allowDecimal = false, secure = false, label, unit, onCommit,
  onDraftChange, onCancel,
}: InputModalProps) {
  const [draft, setDraft] = useState(value);
  const title = label ?? (kind === "number" ? "Number input" : "Text input");
  const updateDraft = (next: string) => {
    setDraft(next);
    onDraftChange?.(next);
  };
  return createPortal(
    <div className="stacked-modal-layer ui-input-modal-layer"
      onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
      <section className={`nested-modal ${kind !== "number" ? "keyboard-modal" : "number-field-modal"}`}
        role="dialog" aria-modal="true" aria-label={title}>
        <ModalTitleBar title={title} closeLabel="Close input" onClose={onCancel}/>
        <ValuePreview kind={kind} label={label} secure={secure} unit={unit} value={draft}/>
        {kind !== "number"
          ? <ModalTextKeyboard value={draft} onChange={updateDraft}
            onEnter={() => onCommit(draft)} onEscape={onCancel}
            multiline={kind === "multiline"}/>
          : <ModalNumberInput value={draft} allowDecimal={allowDecimal}
            onChange={updateDraft} onEnter={() => onCommit(draft)} onEscape={onCancel}/>
        }
      </section>
    </div>,
    document.body,
  );
}
