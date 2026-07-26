/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";
import sharedStyles from "../styles/window-kit.css?raw";

const stylesheets = [
  {
    name: "shared table styles",
    source: sharedStyles,
  },
];

function ruleBodies(source: string, selector: string): string[] {
  const escaped = selector
    .trim()
    .split(/\s+/)
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("\\s*");
  return [...source.matchAll(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "g"))]
    .map((match) => match[1]);
}

describe("DataTable separators", () => {
  it.each(stylesheets)(
    "$name keeps the row border as the only horizontal separator",
    ({ source }) => {
      const tableRules = ruleBodies(source, ".ui-data-table");
      const rowRules = ruleBodies(source, ".ui-data-table-row");
      const cellRules = ruleBodies(source, ".ui-data-table-row > span");

      expect(tableRules.length).toBeGreaterThan(0);
      expect(tableRules.join("\n")).not.toMatch(/(?:repeating-)?linear-gradient/);
      expect(tableRules.join("\n")).not.toMatch(/border-(?:top|bottom)\s*:/);
      expect(rowRules).toHaveLength(1);
      expect(rowRules[0]).toMatch(/border-bottom\s*:\s*1px solid var\(--line\)/);
      expect(cellRules.join("\n")).not.toMatch(/border-(?:top|bottom)\s*:/);
    },
  );
});
