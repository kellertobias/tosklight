import { useEffect, useRef } from "react";
import {
  HardwareEncoderDisplayView,
  type HardwareEncoderDisplayHandle,
  type HardwareEncoderDisplayProps,
} from "@tosklight/ui/encoders";

export function HardwareEncoderDisplay({
  activateOnHardwarePress = false,
  onHardwarePress,
  ...props
}: HardwareEncoderDisplayProps & {
  activateOnHardwarePress?: boolean;
  /** Return true when the application handled the press and Set Value must stay closed. */
  onHardwarePress?: () => boolean;
}) {
  const display = useRef<HardwareEncoderDisplayHandle>(null);
  useEffect(() => {
    if (!activateOnHardwarePress) return;
    const handleEncoder = (event: Event) => {
      const { control, value } = (event as CustomEvent<{ control: string; value?: string }>).detail;
      if (control !== `encode/${props.slot}` || value !== "press") return;
      if (onHardwarePress?.()) return;
      if (props.onEdit) display.current?.activate();
    };
    window.addEventListener("light:encoder-action", handleEncoder);
    return () => window.removeEventListener("light:encoder-action", handleEncoder);
  }, [activateOnHardwarePress, onHardwarePress, props.onEdit, props.slot]);
  return <HardwareEncoderDisplayView ref={display} {...props} />;
}
