import { describe, expect, it } from "vitest";
import { builtIns } from "../components/shell/LeftDock";
import { windowChoices } from "../components/modals/WindowPicker";
import { windowRegistry } from "./WindowRegistry";

describe("Help window registration", () => {
  it("is pane-capable but omitted from the Built-ins dock", () => {
    expect(windowRegistry.help).toBeDefined();
    expect(windowChoices.some(([kind]) => kind === "help")).toBe(true);
    expect(builtIns.some(([kind]) => kind === "help")).toBe(false);
  });
});

describe("Development window registration", () => {
  it("remains routable through developer tooling but is not an operator pane or Built-in choice", () => {
    expect(windowRegistry.development).toBeDefined();
    expect(windowChoices.some(([kind]) => kind === "development")).toBe(false);
    expect(builtIns.some(([kind]) => kind === "development")).toBe(false);
  });
});
