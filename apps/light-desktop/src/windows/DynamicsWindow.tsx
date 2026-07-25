import { WindowScrollArea } from "../components/window-kit";
import type { WindowProps } from "./windowTypes";

export function DynamicsWindow(_: WindowProps) {
  return <div className="dynamics-window">
    <WindowScrollArea emptyState={{
      title: "Dynamics is a future feature",
      description: "Dynamics is currently being conceptualized.",
      icon: "∿",
    }} />
  </div>;
}
