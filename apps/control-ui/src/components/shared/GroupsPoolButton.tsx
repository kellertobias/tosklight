import { useRef } from "react";
import { useApp } from "../../state/AppContext";

export function GroupsPoolButton({ shortcutsVisible, onToggleShortcuts }: { shortcutsVisible: boolean; onToggleShortcuts: () => void }) {
  const { dispatch } = useApp();
  const timer = useRef<number | null>(null);
  const held = useRef(false);
  const cancel = () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
  };
  return <button
    className={`group-shortcut-toggle ${shortcutsVisible ? "active" : ""}`}
    title="Tap to open Groups · hold to show or hide group shortcuts"
    onPointerDown={() => {
      held.current = false;
      timer.current = window.setTimeout(() => {
        held.current = true;
        onToggleShortcuts();
      }, 650);
    }}
    onPointerUp={cancel}
    onPointerCancel={cancel}
    onContextMenu={(event) => event.preventDefault()}
    onClick={() => {
      if (!held.current) dispatch({ type: "OPEN_BUILTIN", kind: "groups" });
      held.current = false;
    }}
  >Groups</button>;
}
