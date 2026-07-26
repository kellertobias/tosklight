import { GridDesktop } from "@tosklight/ui/desktop";
import { GRID_COLUMNS, GRID_ROWS, type DeskModel, type GridRect } from "../../types";
import { useApp } from "../../state/AppContext";
import { Pane } from "./Pane";
import { WindowPicker } from "../modals/WindowPicker";
import { PaneSettingsModal } from "../modals/PaneSettingsModal";

export function DeskGrid({ desk }: { desk: DeskModel }) {
  const { state, dispatch } = useApp();
  const empty = desk.panes.length === 0;
  const openAt = (rect: GridRect) =>
    dispatch({ type: "OPEN_WINDOW_PICKER", rect });
  return <GridDesktop
    id={desk.id}
    name={desk.name}
    dimensions={{ columns: GRID_COLUMNS, rows: GRID_ROWS }}
    editing={Boolean(state.paneSettingsId)}
    empty={empty}
    onOpen={openAt}
  >
    {desk.panes.map((pane) => <Pane key={pane.id} pane={pane} active={state.maximizedPaneId == null || state.maximizedPaneId === pane.id} maximized={state.maximizedPaneId === pane.id} editing={state.paneSettingsId === pane.id} />)}
    <WindowPicker />
    <PaneSettingsModal />
  </GridDesktop>;
}
