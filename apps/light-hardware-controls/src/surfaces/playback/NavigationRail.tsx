import {
  controlSurfaceOscPaths,
  type ProgrammerControlAction,
} from "@tosklight/ui/control-surface-contracts";
import { ControlButton } from "../../components/ControlButton";
import type { SendControl } from "../../controller/types";

interface NavigationRailProps {
  page: number;
  send: SendControl;
}

export function NavigationRail({ page, send }: NavigationRailProps) {
  const programmerKey = (label: "ESCAPE" | "MENU" | "PROG-PLAYBACK") => {
    const action = label.toLowerCase() as ProgrammerControlAction;
    return (
      <ControlButton
        className={`key-${action}`}
        label={label}
        onDown={() => send(controlSurfaceOscPaths.programmer(action), [true])}
        onUp={() => send(controlSurfaceOscPaths.programmer(action), [false])}
      />
    );
  };

  return (
    <aside className="left-rail">
      {programmerKey("ESCAPE")}
      {programmerKey("MENU")}
      {programmerKey("PROG-PLAYBACK")}
      <span className="button-spacer" />
      <ControlButton
        className="key-align"
        label="ALIGN"
        onDown={() => undefined}
        onUp={() => undefined}
      />
      <span className="button-spacer" />
      <button
        type="button"
        onClick={() =>
          send(controlSurfaceOscPaths.page, [Math.max(1, page - 1)])
        }
      >
        PAGE UP
      </button>
      <strong>{page}</strong>
      <button
        type="button"
        onClick={() => send(controlSurfaceOscPaths.page, [page + 1])}
      >
        PAGE DOWN
      </button>
    </aside>
  );
}
