import { describe, expect, it } from "vitest";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";

const parameter = (attribute: string) => ({ attribute, components: [], default: 0, virtual_dimmer: false, capabilities: [] });

function fixture(id: string, shared: boolean, attribute: string): PatchedFixture {
  return {
    fixture_id: id,
    fixture_number: 1,
    name: id,
    universe: 1,
    address: 1,
    layer_id: "default",
    definition: {
      schema_version: 1,
      id: `${id}-definition`,
      revision: 1,
      manufacturer: "Test",
      device_type: "profile",
      name: id,
      model: id,
      mode: "Test",
      footprint: 1,
      heads: [{ index: 3, name: "Main", shared, parameters: [parameter(attribute)] }],
      color_calibration: null,
      physical: {},
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
    } as FixtureDefinition,
    logical_heads: shared ? [] : [{ fixture_id: `${id}-head`, head_index: 3 }],
  };
}

describe("compatibleSpecialDialogActions", () => {
  it("uses lamp control where present and intensity for conventional lamps", () => {
    expect(compatibleSpecialDialogActions([
      fixture("shared", true, "control.lamp"),
      fixture("headed", false, "control.lamp"),
      fixture("dimmer", true, "intensity"),
    ], "control.lamp")).toEqual([
      { fixtureId: "shared", attribute: "control.lamp" },
      { fixtureId: "headed-head", attribute: "control.lamp" },
      { fixtureId: "dimmer", attribute: "intensity" },
    ]);
  });
});
