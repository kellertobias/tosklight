import { ControlButton } from "../../components/ControlButton";
import type { SendControl } from "../../controller/types";
import { oscPaths } from "../../oscPaths";

interface NavigationRailProps {
  page: number;
  send: SendControl;
}

export function NavigationRail({ page, send }: NavigationRailProps) {
  const programmerKey = (label: string) => {
    const action = label.toLowerCase();
    return (
      <ControlButton
        className={`key-${action}`}
        label={label}
        onDown={() => send(oscPaths.programmer(action), [true])}
        onUp={() => send(oscPaths.programmer(action), [false])}
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
      <button onClick={() => send(oscPaths.page, [Math.max(1, page - 1)])}>
        PAGE UP
      </button>
      <strong>{page}</strong>
      <button onClick={() => send(oscPaths.page, [page + 1])}>PAGE DOWN</button>
    </aside>
  );
}
