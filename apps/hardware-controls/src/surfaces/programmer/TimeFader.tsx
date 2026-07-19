import { useState } from "react";
import { TouchFader } from "../../components/TouchFader";
import type { SendControl } from "../../controller/types";

interface TimeFaderProps {
  label: string;
  path: string;
  maximum: number;
  send: SendControl;
}

export function TimeFader({ label, path, maximum, send }: TimeFaderProps) {
  const [value, setValue] = useState(0.15);
  return (
    <TouchFader
      className="time-fader"
      label={label}
      value={value}
      display={`${(value * maximum).toFixed(1)}s`}
      onChange={(next) => {
        setValue(next);
        send(path, [next]);
      }}
    />
  );
}
