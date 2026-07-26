import { useRef } from "react";
import { pointerToGridCell } from "@tosklight/ui/desktop";
import { GRID_COLUMNS, GRID_ROWS, type DeskModel, type GridRect } from "../../types";
import { useApp } from "../../state/AppContext";
import { Pane } from "./Pane";
import { WindowPicker } from "../modals/WindowPicker";
import { PaneSettingsModal } from "../modals/PaneSettingsModal";
import { Button } from "@tosklight/ui";

export function DeskGrid({ desk }: { desk: DeskModel }) {
  const { state, dispatch } = useApp();
  const ref = useRef<HTMLDivElement>(null);
  const empty = desk.panes.length === 0;
  const openAtPointer = (event: React.PointerEvent<HTMLElement>) => {
    const rect = ref.current!.getBoundingClientRect();
    const { x, y } = pointerToGridCell(event.clientX, event.clientY, rect, {
      columns: GRID_COLUMNS,
      rows: GRID_ROWS,
    });
    dispatch({ type: "OPEN_WINDOW_PICKER", rect: { x, y, width: 6, height: 6 } });
  };
  return <div className={`desk-grid ${state.paneSettingsId ? "editing" : ""}`} data-desktop-id={desk.id} data-desktop-name={desk.name} aria-label={`${desk.name} Desktop grid`} ref={ref} onPointerDown={(event) => { if (event.target === event.currentTarget) openAtPointer(event); }}>
    {desk.panes.map((pane) => <Pane key={pane.id} pane={pane} active={state.maximizedPaneId == null || state.maximizedPaneId === pane.id} maximized={state.maximizedPaneId === pane.id} editing={state.paneSettingsId === pane.id} />)}
    {empty && <Button className="empty-desk" onPointerDown={openAtPointer}><b>24 × 18 desktop grid</b><span>Tap a grid cell to open a window</span></Button>}
    <WindowPicker />
    <PaneSettingsModal />
  </div>;
}
