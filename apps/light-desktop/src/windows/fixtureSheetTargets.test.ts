import { describe, expect, it } from "vitest";
import type { FixtureDefinition, PatchedFixture, VisualizationSnapshot } from "../api/types";
import { fixtureSheetTargets, targetValue } from "./fixtureSheetTargets";

const parameter = (attribute: string, value = 0) => ({
  attribute,
  components: [],
  default: value,
  virtual_dimmer: false,
  capabilities: [],
});

function fixture(): PatchedFixture {
  return {
    fixture_id: "master",
    fixture_number: 100,
    name: "Bar",
    universe: 1,
    address: 1,
    definition: {
      schema_version: 1,
      id: "definition",
      revision: 1,
      manufacturer: "Test",
      device_type: "pixel fixture",
      name: "Bar",
      model: "Bar",
      mode: "2 cells",
      footprint: 4,
      heads: [
        { index: 8, name: "Base", shared: true, parameters: [parameter("tilt", 0.5)] },
        { index: 2, name: "Left", shared: false, parameters: [parameter("intensity")] },
        { index: 7, name: "Right", shared: false, parameters: [parameter("intensity")] },
      ],
      color_calibration: null,
      physical: {},
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
    } as FixtureDefinition,
    logical_heads: [
      { fixture_id: "right", head_index: 7 },
      { fixture_id: "left", head_index: 2 },
    ],
  };
}

describe("fixture sheet targets", () => {
  it("shows an exact master row followed by definition-ordered child rows", () => {
    const targets = fixtureSheetTargets(fixture());
    expect(targets.map((target) => [target.displayId, target.fixtureId, target.name, target.indented])).toEqual([
      ["100.0", "master", "Bar · Master", false],
      ["100.1", "left", "Bar · Left", true],
      ["100.2", "right", "Bar · Right", true],
    ]);
  });

  it("uses the fixture ID for a master-only row", () => {
    expect(fixtureSheetTargets(fixture(), "no-sub-heads").map((target) => [target.displayId, target.fixtureId])).toEqual([
      ["100", "master"],
    ]);
  });

  it("shows unindented subheads when master rows are excluded", () => {
    expect(fixtureSheetTargets(fixture(), "no-master-heads").map((target) => [target.displayId, target.fixtureId, target.indented])).toEqual([
      ["100.1", "left", false],
      ["100.2", "right", false],
    ]);
  });

  it("reads only the exact target's live value and otherwise uses its own default", () => {
    const targets = fixtureSheetTargets(fixture());
    const snapshot = {
      values: [
        { fixture_id: "master", attribute: "tilt", value: { kind: "normalized", value: 0.75 } },
        { fixture_id: "master", attribute: "intensity", value: { kind: "normalized", value: 1 } },
        { fixture_id: "left", attribute: "intensity", value: { kind: "normalized", value: 0.4 } },
      ],
    } as VisualizationSnapshot;
    expect(targetValue(snapshot, targets[0], "tilt")).toBe(0.75);
    expect(targetValue(snapshot, targets[1], "intensity")).toBe(0.4);
    expect(targetValue(snapshot, targets[2], "intensity")).toBe(0);
    expect(targetValue(snapshot, targets[0], "intensity")).toBe(0);
  });

  it("keeps an ordinary fixture on its bare ID", () => {
    const ordinary = fixture();
    ordinary.fixture_number = 42;
    ordinary.logical_heads = [];
    ordinary.definition.heads = [{ index: 0, name: "Main", shared: true, parameters: [] }];
    expect(fixtureSheetTargets(ordinary)[0].displayId).toBe(42);
  });

  it("shows a visual-only fixture in the reserved 0.x namespace", () => {
    const visual = fixture();
    visual.fixture_number = null;
    visual.virtual_fixture_number = 3;
    visual.logical_heads = [];
    visual.definition.heads = [{ index: 0, name: "Visual", shared: true, parameters: [] }];
    expect(fixtureSheetTargets(visual)[0].displayId).toBe("0.3");
  });
});
