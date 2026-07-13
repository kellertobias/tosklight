import { describe, expect, it } from "vitest";
import { groupShortcutCount } from "./GroupStrip";

describe("groupShortcutCount", () => {
  it("uses the same 88px cells and 2px gaps as the pool grids", () => {
    expect(groupShortcutCount(88)).toBe(1);
    expect(groupShortcutCount(178)).toBe(2);
    expect(groupShortcutCount(898)).toBe(10);
    expect(groupShortcutCount(988)).toBe(11);
  });

  it("always keeps one shortcut available in very narrow panes", () => {
    expect(groupShortcutCount(0)).toBe(1);
  });
});
