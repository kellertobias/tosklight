import { useRef, type ReactNode } from "react";
import { WindowHeader, type WindowAction, type WindowInfo } from "../window-kit";
import {
  DEFAULT_GRID_COLUMNS,
  DEFAULT_GRID_ROWS,
  gridRectStyle,
  moveGridRect,
  pointerToGridCell,
  pointerToGridEdge,
  resizeGridRect,
  type GridDimensions,
  type GridRect,
} from "./GridGeometry";

export interface PaneViewModel extends GridRect {
  id: string;
  title: string;
  type?: string;
}

export interface PaneViewProps {
  pane: PaneViewModel;
  active?: boolean;
  maximized?: boolean;
  editing?: boolean;
  dimensions?: GridDimensions;
  info?: WindowInfo;
  toolbar?: ReactNode;
  actions?: WindowAction[][];
  settings?: boolean;
  onSettings?: () => void;
  onTitleClick?: () => void;
  titleActionLabel?: string;
  acceptRect?: (candidate: GridRect) => boolean;
  onRectChange?: (rect: GridRect) => void;
  children: ReactNode;
}

export function PaneView({
  pane,
  active = true,
  maximized = false,
  editing = false,
  dimensions = { columns: DEFAULT_GRID_COLUMNS, rows: DEFAULT_GRID_ROWS },
  info,
  toolbar,
  actions,
  settings,
  onSettings,
  onTitleClick,
  titleActionLabel,
  acceptRect = () => true,
  onRectChange,
  children,
}: PaneViewProps) {
  const drag = useRef<number | null>(null);
  const resize = useRef<number | null>(null);
  const requestRect = (candidate: GridRect) => {
    if (acceptRect(candidate)) onRectChange?.(candidate);
  };
  return (
    <article
      className={`desk-pane ${maximized ? "maximized" : ""} ${editing ? "editing" : ""}`}
      role="region"
      aria-label={`${pane.title} pane`}
      aria-expanded={maximized}
      aria-hidden={!active || undefined}
      tabIndex={-1}
      data-pane-id={pane.id}
      data-pane-type={pane.type}
      data-grid-column={pane.x}
      data-grid-row={pane.y}
      data-grid-width={pane.width}
      data-grid-height={pane.height}
      style={gridRectStyle(pane)}
      onPointerDown={(event) => event.currentTarget.focus()}
    >
      <WindowHeader
        title={pane.title}
        info={info}
        toolbar={toolbar}
        actions={actions}
        settings={settings}
        onSettings={onSettings ? () => onSettings() : undefined}
        onTitleClick={onTitleClick}
        titleActionLabel={titleActionLabel}
        dragHandleProps={{
          className: "pane-drag-handle",
          onPointerDown: (event) => {
            if ((event.target as HTMLElement).closest("button")) return;
            drag.current = event.pointerId;
            event.currentTarget.setPointerCapture(event.pointerId);
          },
          onPointerMove: (event) => {
            if (drag.current !== event.pointerId) return;
            const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect();
            if (!grid) return;
            requestRect(moveGridRect(pane, pointerToGridCell(event.clientX, event.clientY, grid, dimensions), dimensions));
          },
          onPointerUp: () => { drag.current = null; },
          onPointerCancel: () => { drag.current = null; },
        }}
      />
      <div className="pane-content">{children}</div>
      {!maximized && (
        <div
          className="pane-resize-handle"
          aria-label={`Resize ${pane.title}`}
          onPointerDown={(event) => {
            event.stopPropagation();
            resize.current = event.pointerId;
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={(event) => {
            if (resize.current !== event.pointerId) return;
            const grid = event.currentTarget.closest(".desk-grid")?.getBoundingClientRect();
            if (!grid) return;
            requestRect(resizeGridRect(pane, pointerToGridEdge(event.clientX, event.clientY, grid, dimensions), dimensions));
          }}
          onPointerUp={() => { resize.current = null; }}
          onPointerCancel={() => { resize.current = null; }}
        />
      )}
    </article>
  );
}
