import { useRef, useState, type CSSProperties } from "react";
import {
  gridRectStyle,
  moveGridRect,
  pointerToGridCell,
  pointerToGridEdge,
  resizeGridRect,
} from "@tosklight/ui/desktop";
import type { PaneModel } from "../../types";
import { useApp } from "../../state/AppContext";
import { windowRegistry } from "../../windows/WindowRegistry";
import {
  useProgrammingCommandLineActions,
  useProgrammingDeleteCommandActive,
  useProgrammingSelectionView,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { SourceLegend } from "../shared/SourceLegend";
import { PaneChromeProvider } from "./PaneChromeContext";
import { requestPaneRemoval } from "./paneRemovalGuard";

export function Pane({
  pane,
  active,
  maximized,
  editing,
}: {
  pane: PaneModel;
  active: boolean;
  maximized: boolean;
  editing: boolean;
}) {
  const { state, dispatch } = useApp();
  const selection = useProgrammingSelectionView(active && (pane.kind === "stage" || pane.kind === "fixtures"));
  const commandLineActions = useProgrammingCommandLineActions();
  const deleteArmed = useProgrammingDeleteCommandActive();
  const drag = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const resize = useRef<{ pointerId: number; left: number; top: number } | null>(null);
  const lastFollowToggle = useRef(0);
  const [chromeInfo, setChromeInfo] = useState<HTMLSpanElement | null>(null);
  const [chromeToolbar, setChromeToolbar] = useState<HTMLSpanElement | null>(null);
  const Window = windowRegistry[pane.kind];
  const style = gridRectStyle(pane) as CSSProperties;
  const stageActions = pane.kind === "stage" ? [
    [{ id: "follow", label: "Follow Preload", active: Boolean(pane.followPreload), onClick: () => { const now = performance.now(); if (now - lastFollowToggle.current < 400) return; lastFollowToggle.current = now; dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: !pane.followPreload }); } }],
    [{ id: "groups", label: "Groups", onClick: () => dispatch({ type: "OPEN_GROUPS_FROM_STAGE", origin: "desk" }) }],
  ] : [];
  const gridDimensions = { columns: 24, rows: 18 };
  const removeFromDelete = () => {
    if (!deleteArmed || !requestPaneRemoval(pane.id)) return;
    dispatch({ type: "REMOVE_PANE", id: pane.id });
    void commandLineActions?.reset();
  };
  const moveFromPointer = (event: React.PointerEvent<HTMLElement>) => {
    const activeDrag = drag.current;
    if (!activeDrag || activeDrag.pointerId !== event.pointerId) return;
    const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect();
    if (!grid) return;
    const candidate = moveGridRect(
      pane,
      pointerToGridCell(event.clientX, event.clientY, grid, gridDimensions),
      gridDimensions,
    );
    if (candidate.x !== pane.x || candidate.y !== pane.y) {
      dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { x: candidate.x, y: candidate.y } });
    }
  };
  const resizeFromPointer = (event: React.PointerEvent<HTMLElement>) => {
    const activeResize = resize.current;
    if (!activeResize || activeResize.pointerId !== event.pointerId) return;
    const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect();
    if (!grid) return;
    const candidate = resizeGridRect(
      pane,
      pointerToGridEdge(event.clientX, event.clientY, grid, gridDimensions),
      gridDimensions,
    );
    dispatch({
      type: "SET_PANE_RECT",
      id: pane.id,
      rect: { width: candidate.width, height: candidate.height },
    });
  };
  return (
    <article
      className={`desk-pane ${maximized ? "maximized" : ""} ${editing ? "editing" : ""}`}
      role="region"
      aria-label={`${pane.title} pane`}
      aria-expanded={maximized}
      tabIndex={-1}
      data-pane-id={pane.id}
      data-pane-type={pane.kind}
      data-grid-column={pane.x}
      data-grid-row={pane.y}
      data-grid-width={pane.width}
      data-grid-height={pane.height}
      style={style}
      onPointerDown={(event) => event.currentTarget.focus()}
    >
    <WindowHeader title={pane.kind === "file_manager" ? "File Manager" : pane.title} info={pane.kind === "file_manager" ? { primary: "Browse and manage files", secondary: <span className="pane-chrome-info-target" ref={setChromeInfo} /> } : pane.kind === "text_editor" ? { primary: <span className="pane-chrome-info-target" ref={setChromeInfo} /> } : pane.kind === "stage" ? { primary: `${selection?.selected.length ?? 0} selected`, secondary: "Tap to select · Shift for range" } : pane.kind === "fixtures" ? { primary: `${selection?.selected.length ?? 0} selected`, secondary: <SourceLegend /> } : undefined} toolbar={pane.kind === "file_manager" || pane.kind === "text_editor" ? <span className="pane-chrome-toolbar-target" ref={setChromeToolbar} /> : undefined} actions={stageActions} settings onSettings={() => dispatch({ type: "SET_PANE_SETTINGS", id: pane.id })} onTitleClick={deleteArmed ? removeFromDelete : undefined} titleActionLabel={deleteArmed ? `Remove ${pane.kind === "file_manager" ? "File Manager" : pane.title} pane` : undefined} dragHandleProps={{ className: "pane-drag-handle", onPointerDown: (event) => { if ((event.target as HTMLElement).closest("button")) return; const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; drag.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }, onPointerMove: moveFromPointer, onPointerUp: () => { drag.current = null; }, onPointerCancel: () => { drag.current = null; } }} />
      <div className="pane-content">
        <PaneChromeProvider value={{ info: chromeInfo, toolbar: chromeToolbar }}>
	          <Window active={active} compact paneId={pane.id} showGroupShortcuts={Boolean(pane.showGroupShortcuts)} showCueSidebar={pane.showCueSidebar ?? true} cueListSource={pane.cueListSource ?? "fixed"} fixedCueListNumber={pane.fixedCueListNumber} stageView={pane.stageView ?? state.stageView} followPreload={Boolean(pane.followPreload)} showBeamGuides={pane.showBeamGuides ?? true} presetFamily={pane.presetFamily ?? state.presetFamily} presetPoolColors={pane.presetPoolColors ?? true} developmentView={pane.developmentView ?? "forms"} />
        </PaneChromeProvider>
      </div>
      {!maximized && <div className="pane-resize-handle" aria-label={`Resize ${pane.title}`} onPointerDown={(event) => { event.stopPropagation(); const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect(); if (!grid) return; resize.current = { pointerId: event.pointerId, left: grid.left, top: grid.top }; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={resizeFromPointer} onPointerUp={() => { resize.current = null; }} onPointerCancel={() => { resize.current = null; }} />}
    </article>
  );
}
