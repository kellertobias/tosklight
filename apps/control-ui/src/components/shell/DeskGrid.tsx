import { useRef } from "react";
import { GRID_COLUMNS, GRID_ROWS, type DeskModel, type GridRect } from "../../types";
import { useApp } from "../../state/AppContext";
import { Pane } from "./Pane";
import { WindowPicker } from "../modals/WindowPicker";
import { PaneSettingsModal } from "../modals/PaneSettingsModal";

export function DeskGrid({ desk }: { desk: DeskModel }) {
  const { state, dispatch } = useApp();
  const ref = useRef<HTMLDivElement>(null);
  const empty = desk.panes.length === 0;
  const openAtPointer = (event: React.PointerEvent<HTMLButtonElement>) => {
    const rect = ref.current!.getBoundingClientRect();
    const x = Math.max(1, Math.min(GRID_COLUMNS, Math.floor((event.clientX - rect.left) / rect.width * GRID_COLUMNS) + 1));
    const y = Math.max(1, Math.min(GRID_ROWS, Math.floor((event.clientY - rect.top) / rect.height * GRID_ROWS) + 1));
    dispatch({ type: "OPEN_WINDOW_PICKER", rect: { x, y, width: 6, height: 6 } });
  };
  return <div className={`desk-grid ${state.paneSettingsId ? "editing" : ""}`} ref={ref}>
    {desk.panes.map((pane) => <Pane key={pane.id} pane={pane} maximized={state.maximizedPaneId === pane.id} editing={state.paneSettingsId === pane.id} />)}
    {empty && <button className="empty-desk" onPointerDown={openAtPointer}><b>24 × 18 desk grid</b><span>Tap a grid cell to open a window</span></button>}
    <WindowPicker />
    <PaneSettingsModal />
  </div>;
}
