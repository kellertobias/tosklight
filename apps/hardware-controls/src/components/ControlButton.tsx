import { useRef, useState, type CSSProperties } from "react";
import type { SoftwareKey } from "../../../shared/programmerKeypad";
import { darkLamp, type Lamp } from "../controller/types";

type HardwareKey =
  | SoftwareKey
  | "HIGH"
  | "PREV"
  | "NEXT"
  | "ALL"
  | "RECORD"
  | "PRELOAD GO";

interface ControlButtonProps {
  label: string;
  lamp?: Lamp;
  onDown: () => void;
  onUp: () => void;
  className?: string;
  style?: CSSProperties;
  keypadKey?: HardwareKey;
  showHoldFeedback?: boolean;
}

export function ControlButton({
  label,
  lamp = darkLamp,
  onDown,
  onUp,
  className = "",
  style,
  keypadKey,
  showHoldFeedback = true,
}: ControlButtonProps) {
  const timer = useRef<number | undefined>(undefined);
  const [long, setLong] = useState(false);
  const [pressed, setPressed] = useState(false);

  const release = () => {
    clearTimeout(timer.current);
    onUp();
    window.setTimeout(() => setPressed(false), 90);
  };

  return (
    <button
      className={`control-button ${lamp.state} ${lamp.state === "on" && lamp.bpm ? "beat" : ""} ${pressed ? "local-pressed" : ""} ${className}`}
      data-keypad-key={keypadKey}
      style={{
        ...style,
        "--lamp": lamp.color,
        "--bpm": lamp.bpm ?? 60,
      } as CSSProperties}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        setPressed(true);
        setLong(false);
        timer.current = window.setTimeout(() => setLong(true), 650);
        onDown();
      }}
      onPointerUp={release}
      onPointerCancel={release}
    >
      <span>{label}</span>
      {showHoldFeedback && long && <i>LONG</i>}
    </button>
  );
}
