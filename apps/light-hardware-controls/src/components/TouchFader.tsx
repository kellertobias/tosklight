import {
  useEffect,
  useRef,
  type CSSProperties,
  type InputHTMLAttributes,
} from "react";

function WheelSafeRange(props: InputHTMLAttributes<HTMLInputElement>) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const range = input.current;
    if (!range) return;
    const rejectWheel = (event: WheelEvent) => {
      event.preventDefault();
      range.blur();
    };
    range.addEventListener("wheel", rejectWheel, { passive: false });
    return () => range.removeEventListener("wheel", rejectWheel);
  }, []);
  return <input {...props} ref={input} type="range" />;
}

interface TouchFaderProps {
  label: string;
  value: number;
  display: string;
  onChange: (value: number) => void;
  className?: string;
}

export function TouchFader({
  label,
  value,
  display,
  onChange,
  className = "",
}: TouchFaderProps) {
  return (
    <label
      className={`touch-fader ${className}`}
      style={{ "--fader-level": value } as CSSProperties}
    >
      <span>{label}</span>
      <strong>{display}</strong>
      <WheelSafeRange
        min="0"
        max="1"
        step=".001"
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}
