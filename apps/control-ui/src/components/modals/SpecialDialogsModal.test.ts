import { describe, expect, it } from "vitest";
import type { ControlActionSemantic, FixtureDefinition, PatchedFixture } from "../../api/types";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";

function fixture(
  id: string,
  action?: { id: string; name: string; semantic?: ControlActionSemantic },
): PatchedFixture {
  const modeId = `${id}-mode`;
  return {
    fixture_id: id,
    fixture_number: 1,
    name: id,
    universe: 1,
    address: 1,
    layer_id: "default",
    definition: {
      schema_version: action ? 2 : 1,
      id: `${id}-definition`,
      revision: 1,
      manufacturer: "Test",
      device_type: "profile",
      name: id,
      model: id,
      mode: "Test",
      mode_id: action ? modeId : null,
      footprint: 1,
      heads: [{ index: 0, name: "Main", shared: true, parameters: [{ attribute: "intensity", components: [], default: 0, virtual_dimmer: false, capabilities: [] }] }],
      color_calibration: null,
      physical: {},
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
      profile_snapshot: action ? {
        schema_version: 2,
        id: `${id}-profile`,
        revision: 1,
        manufacturer: "Test",
        name: id,
        short_name: id,
        fixture_type: "Moving light",
        notes: "",
        photograph_asset: null,
        stage_icon_asset: null,
        model_asset: null,
        physical: { width_millimetres: null, height_millimetres: null, depth_millimetres: null, weight_kilograms: null, power_watts: null },
        modes: [{ id: modeId, name: "Test", notes: "", splits: [], heads: [], channels: [], color_systems: [], control_actions: [{ id: action.id, name: action.name, semantic: action.semantic ?? "custom", kind: "timed_pulse", duration_millis: 1000, assignments: [] }], geometry: { nodes: [], emitters: [] } }],
        hazardous: false,
        direct_control_protocols: [],
        signal_loss_policy: { type: "hold_last" },
        reserved_source: null,
      } : null,
    } as FixtureDefinition,
    logical_heads: [],
  };
}

describe("compatibleSpecialDialogActions", () => {
  it("only returns typed Lamp On actions and never treats intensity as lamp control", () => {
    expect(compatibleSpecialDialogActions([
      fixture("arc", { id: "strike", name: "Strike", semantic: "lamp_on" }),
      fixture("legacy-arc", { id: "legacy-strike", name: "Lamp On" }),
      fixture("dimmer"),
      fixture("led"),
    ], "lamp_on")).toEqual([
      { fixtureId: "arc", actionId: "strike", kind: "timed_pulse" },
      { fixtureId: "legacy-arc", actionId: "legacy-strike", kind: "timed_pulse" },
    ]);
  });

  it("filters typed actions to the selected physical or logical fixture", () => {
    const first = fixture("first", { id: "reset-first", name: "Reset", semantic: "reset" });
    const second = fixture("second", { id: "reset-second", name: "Reset", semantic: "reset" });
    second.logical_heads = [{ fixture_id: "second-head", head_index: 0 }];
    expect(compatibleSpecialDialogActions([first, second], "reset", ["second-head"]))
      .toEqual([{ fixtureId: "second", actionId: "reset-second", kind: "timed_pulse" }]);
  });
});
