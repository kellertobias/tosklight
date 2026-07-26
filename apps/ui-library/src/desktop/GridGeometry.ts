import type { CSSProperties } from "react";

export const DEFAULT_GRID_COLUMNS = 24;
export const DEFAULT_GRID_ROWS = 18;

export interface GridDimensions {
  columns: number;
  rows: number;
}

export interface GridPoint {
  x: number;
  y: number;
}

export interface GridRect extends GridPoint {
  width: number;
  height: number;
}

const defaultDimensions: GridDimensions = {
  columns: DEFAULT_GRID_COLUMNS,
  rows: DEFAULT_GRID_ROWS,
};

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(maximum, value));
}

export function pointerToGridCell(
  clientX: number,
  clientY: number,
  bounds: Pick<DOMRect, "left" | "top" | "width" | "height">,
  dimensions: GridDimensions = defaultDimensions,
): GridPoint {
  return {
    x: clamp(
      Math.floor((clientX - bounds.left) / bounds.width * dimensions.columns) + 1,
      1,
      dimensions.columns,
    ),
    y: clamp(
      Math.floor((clientY - bounds.top) / bounds.height * dimensions.rows) + 1,
      1,
      dimensions.rows,
    ),
  };
}

export function pointerToGridEdge(
  clientX: number,
  clientY: number,
  bounds: Pick<DOMRect, "left" | "top" | "width" | "height">,
  dimensions: GridDimensions = defaultDimensions,
): GridPoint {
  return {
    x: clamp(
      Math.ceil((clientX - bounds.left) / bounds.width * dimensions.columns),
      1,
      dimensions.columns,
    ),
    y: clamp(
      Math.ceil((clientY - bounds.top) / bounds.height * dimensions.rows),
      1,
      dimensions.rows,
    ),
  };
}

export function constrainGridRect(
  rect: GridRect,
  dimensions: GridDimensions = defaultDimensions,
): GridRect {
  const x = clamp(rect.x, 1, dimensions.columns);
  const y = clamp(rect.y, 1, dimensions.rows);
  return {
    x,
    y,
    width: clamp(rect.width, 1, dimensions.columns - x + 1),
    height: clamp(rect.height, 1, dimensions.rows - y + 1),
  };
}

export function moveGridRect(
  rect: GridRect,
  point: GridPoint,
  dimensions: GridDimensions = defaultDimensions,
): GridRect {
  return constrainGridRect({
    ...rect,
    x: clamp(point.x, 1, dimensions.columns - rect.width + 1),
    y: clamp(point.y, 1, dimensions.rows - rect.height + 1),
  }, dimensions);
}

export function resizeGridRect(
  rect: GridRect,
  point: GridPoint,
  dimensions: GridDimensions = defaultDimensions,
): GridRect {
  const right = clamp(point.x, rect.x, dimensions.columns);
  const bottom = clamp(point.y, rect.y, dimensions.rows);
  return constrainGridRect({
    ...rect,
    width: right - rect.x + 1,
    height: bottom - rect.y + 1,
  }, dimensions);
}

export function gridRectStyle(rect: GridRect): CSSProperties {
  return {
    gridColumn: `${rect.x} / span ${rect.width}`,
    gridRow: `${rect.y} / span ${rect.height}`,
  };
}

export function gridRectsOverlap(left: GridRect, right: GridRect) {
  return !(
    left.x + left.width <= right.x
    || right.x + right.width <= left.x
    || left.y + left.height <= right.y
    || right.y + right.height <= left.y
  );
}
