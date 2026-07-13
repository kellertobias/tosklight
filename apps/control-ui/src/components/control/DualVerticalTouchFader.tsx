import { useState } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";
import { VerticalTouchFader } from "./VerticalTouchFader";

export type DualEncoderValue = { label: string; value: number; maximum: number; display: string; onChange: (value: number) => void };

export function DualVerticalTouchFader({ encoder, primary, secondary }: { encoder: string; primary: DualEncoderValue; secondary: DualEncoderValue }) {
  const server = useServer();
  const { state } = useApp();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  const [editing, setEditing] = useState<"primary" | "secondary" | null>(null);
  const [inputValue, setInputValue] = useState("");
  const selected = editing === "secondary" ? secondary : primary;
  const select = (target: "primary" | "secondary") => {
    const value = target === "primary" ? primary : secondary;
    setEditing(target);
    setInputValue(String(Number(value.value.toFixed(1))));
  };
  const submit = () => {
    const next = Math.max(0, Math.min(selected.maximum, Number(inputValue)));
    if (Number.isFinite(next)) selected.onChange(next);
    setEditing(null);
  };
  if (!hardware) return <div className="dual-touch-encoder" aria-label={`${encoder}: ${primary.label}, press-turn ${secondary.label}`}>
    <VerticalTouchFader label={`${encoder} · ${primary.label}`} value={primary.value} maximum={primary.maximum} display={primary.display} directInput onChange={primary.onChange}/>
    <VerticalTouchFader label={`Press-turn · ${secondary.label}`} value={secondary.value} maximum={secondary.maximum} display={secondary.display} directInput onChange={secondary.onChange}/>
  </div>;
  return <>
    <Button className="dual-hardware-encoder" aria-label={`${encoder}: edit ${primary.label} or ${secondary.label}`} onClick={() => select("primary")}>
      <small>{encoder}</small><span><b>{primary.label}</b><strong>{primary.display}</strong></span><span><b>{secondary.label}</b><strong>{secondary.display}</strong></span>
    </Button>
    {editing && createPortal(<div className="stacked-modal-layer" onClick={(event) => event.stopPropagation()} onPointerDown={(event) => event.target === event.currentTarget && setEditing(null)}><section className="nested-modal direct-value-modal dual-encoder-modal" role="dialog" aria-modal="true" aria-label={`${encoder} value`}>
      <Button className="modal-close" aria-label="Close encoder value" onClick={() => setEditing(null)}>×</Button>
      <nav aria-label={`${encoder} value selection`}><Button className={editing === "primary" ? "active" : ""} onClick={() => select("primary")}>{primary.label}<small>Turn</small></Button><Button className={editing === "secondary" ? "active" : ""} onClick={() => select("secondary")}>{secondary.label}<small>Press-turn</small></Button></nav>
      <h3>{selected.label}</h3><strong>{inputValue || "0"}</strong><ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submit} onEscape={() => setEditing(null)} replaceOnFirstInput/>
    </section></div>, document.body)}
  </>;
}
