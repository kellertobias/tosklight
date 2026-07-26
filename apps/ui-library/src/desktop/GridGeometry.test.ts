import { describe, expect, it } from "vitest";
import {
  constrainGridRect,
  gridRectsOverlap,
  moveGridRect,
  pointerToGridCell,
  pointerToGridEdge,
  resizeGridRect,
} from "./GridGeometry";

const bounds = { left: 100, top: 50, width: 2400, height: 1800 };

describe("24 × 18 grid geometry", () => {
  it("maps pointer coordinates to one-based cells and clamps edges", () => {
    expect(pointerToGridCell(100, 50, bounds)).toEqual({ x: 1, y: 1 });
    expect(pointerToGridCell(2499, 1849, bounds)).toEqual({ x: 24, y: 18 });
    expect(pointerToGridCell(-100, 9999, bounds)).toEqual({ x: 1, y: 18 });
    expect(pointerToGridEdge(200, 150, bounds)).toEqual({ x: 1, y: 1 });
  });

  it("constrains move and resize candidates inside the real grid", () => {
    const pane = { x: 3, y: 4, width: 6, height: 5 };
    expect(moveGridRect(pane, { x: 24, y: 18 })).toEqual({ x: 19, y: 14, width: 6, height: 5 });
    expect(resizeGridRect(pane, { x: 24, y: 18 })).toEqual({ x: 3, y: 4, width: 22, height: 15 });
    expect(constrainGridRect({ x: 30, y: -2, width: 9, height: 20 })).toEqual({ x: 24, y: 1, width: 1, height: 18 });
  });

  it("makes overlap policy explicit", () => {
    expect(gridRectsOverlap(
      { x: 1, y: 1, width: 6, height: 6 },
      { x: 6, y: 6, width: 6, height: 6 },
    )).toBe(true);
    expect(gridRectsOverlap(
      { x: 1, y: 1, width: 6, height: 6 },
      { x: 7, y: 1, width: 6, height: 6 },
    )).toBe(false);
  });
});
