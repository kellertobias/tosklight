import { useRef, type PointerEvent, type ReactNode } from "react";
import { Button } from "../controls";
import {
  DEFAULT_GRID_COLUMNS,
  DEFAULT_GRID_ROWS,
  pointerToGridCell,
  type GridDimensions,
  type GridRect,
} from "./GridGeometry";

export interface GridDesktopProps {
  id: string;
  name: string;
  editing?: boolean;
  empty?: boolean;
  dimensions?: GridDimensions;
  children?: ReactNode;
  onOpen?: (rect: GridRect) => void;
}
export function GridDesktop({
  id,
  name,
  editing = false,
  empty = false,
  dimensions = { columns: DEFAULT_GRID_COLUMNS, rows: DEFAULT_GRID_ROWS },
  children,
  onOpen,
}: GridDesktopProps) {
  const host = useRef<HTMLDivElement>(null);
  const openAtPointer = (event: PointerEvent<HTMLElement>) => {
    const bounds = host.current?.getBoundingClientRect();
    if (!bounds) return;
    const point = pointerToGridCell(event.clientX, event.clientY, bounds, dimensions);
    onOpen?.({ ...point, width: 6, height: 6 });
  };
  return (
    <div
      ref={host}
      className={`desk-grid ${editing ? "editing" : ""}`}
      style={{
        "--desktop-grid-columns": dimensions.columns,
        "--desktop-grid-rows": dimensions.rows,
      } as React.CSSProperties}
      data-desktop-id={id}
      data-desktop-name={name}
      data-ui-component="grid-desktop"
      aria-label={`${name} Desktop grid`}
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) openAtPointer(event);
      }}
    >
      {children}
      {empty && (
        <Button className="empty-desk" onPointerDown={openAtPointer}>
          <b>{dimensions.columns} × {dimensions.rows} desktop grid</b>
          <span>Tap a grid cell to open a window</span>
        </Button>
      )}
    </div>
  );
}
