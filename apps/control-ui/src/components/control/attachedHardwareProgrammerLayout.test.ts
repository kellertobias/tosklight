import { describe, expect, it } from "vitest";
import { numericPadLayout } from "../../../../shared/programmerKeypad";
import {
  attachedHighlightKeys,
  attachedKeypadContentRowOffset,
  attachedProgrammerActionLayout,
} from "../../../../hardware-controls/src/programmerLayout";
import { oscPaths } from "../../../../hardware-controls/src/oscPaths";

describe("attached hardware Programmer layout contract", () => {
  it("aligns HIGH, PREV, NEXT, and ALL directly above GRP, CUE, TIME, and DIV", () => {
    expect(attachedHighlightKeys.map(({ label, action, column, row }) => ({ label, action, column, row }))).toEqual([
      { label: "HIGH", action: "toggle", column: 1, row: 1 },
      { label: "PREV", action: "previous", column: 2, row: 1 },
      { label: "NEXT", action: "next", column: 3, row: 1 },
      { label: "ALL", action: "all", column: 4, row: 1 },
    ]);
    expect(oscPaths.highlight("all")).toBe("highlight/all");

    const targets = ["GRP", "CUE", "TIME", "DIV"];
    for (const [index, key] of targets.entries()) {
      const keypadKey = numericPadLayout.find((item) => item.key === key);
      expect(keypadKey).toMatchObject({ section: "numbers", column: index + 4, row: 1 });
      expect((keypadKey?.column ?? 0) - 3).toBe(attachedHighlightKeys[index].column);
      expect((keypadKey?.row ?? 0) + attachedKeypadContentRowOffset).toBe(attachedHighlightKeys[index].row + 1);
    }
  });

  it("puts RECORD and PRELOAD GO in the two-column by two-row command area", () => {
    expect(attachedProgrammerActionLayout).toEqual({
      record: { column: 1, row: 1, rowSpan: 2 },
      preload: { column: 2, row: 1, rowSpan: 2 },
    });

    const commandRows = numericPadLayout
      .filter((item) => item.section === "commands")
      .map((item) => item.row + attachedKeypadContentRowOffset);
    expect(Math.min(...commandRows)).toBe(3);
    expect(Math.max(...commandRows)).toBe(6);
  });
});
