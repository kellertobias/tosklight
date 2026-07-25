import { useRef } from "react";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";

export function GroupsPoolButton({ shortcutsVisible, onToggleShortcuts, fromStage = false, stageOrigin = "builtin" }: { shortcutsVisible: boolean; onToggleShortcuts: () => void; fromStage?: boolean; stageOrigin?: "builtin" | "desk" }) {
  const { dispatch } = useApp();
  const timer = useRef<number | null>(null);
  const held = useRef(false);
  const suppressUntil = useRef(0);
  const cancel = () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
  };
  return <Button
    className={`group-shortcut-toggle ${shortcutsVisible ? "active" : ""}`}
    title="Tap to open Groups · hold to show or hide group shortcuts"
    onPointerDown={() => {
      held.current = false;
      timer.current = window.setTimeout(() => {
        held.current = true;
        suppressUntil.current = performance.now() + 1000;
        onToggleShortcuts();
      }, 650);
    }}
    onPointerUp={cancel}
    onPointerCancel={cancel}
    onContextMenu={(event) => event.preventDefault()}
    onClick={() => {
      if (!held.current && performance.now() >= suppressUntil.current) dispatch(fromStage ? { type: "OPEN_GROUPS_FROM_STAGE", origin: stageOrigin } : { type: "OPEN_BUILTIN", kind: "groups" });
      held.current = false;
    }}
  >Groups</Button>;
}
