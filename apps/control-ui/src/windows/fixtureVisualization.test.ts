import { describe, expect, it } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import { fixtureValue } from "./fixtureVisualization";

const fixture = {
  fixture_id: "physical",
  universe: 1,
  address: 1,
  logical_heads: [{ fixture_id: "head-1", head_index: 1 }],
  definition: {
    schema_version: 1, id: "definition", revision: 1, manufacturer: "Test",
    device_type: "moving light", name: "Mover", model: "Mover", mode: "Mode",
    footprint: 2, color_calibration: null, physical: {}, hazardous: false,
    direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {},
    heads: [{ index: 1, name: "Head", shared: false, parameters: [
      { attribute: "intensity", components: [], default: 0.15, virtual_dimmer: false, capabilities: [] },
      { attribute: "pan", components: [], default: 0.35, virtual_dimmer: false, capabilities: [] },
    ] }],
  },
} satisfies PatchedFixture;

describe("fixture visualization values", () => {
  it("uses fixture defaults instead of demo state when output has no contribution", () => {
    expect(fixtureValue(null, fixture, "intensity")).toBe(0.15);
    expect(fixtureValue(null, fixture, "pan")).toBe(0.35);
  });

  it("resolves a logical head's live value for its physical lamp", () => {
    const snapshot = {
      revision: 1, generated_at: "2026-07-12T00:00:00Z", grand_master: 1, blackout: false,
      values: [{ fixture_id: "head-1", attribute: "pan", value: { kind: "normalized", value: 0.8 } }],
    } satisfies VisualizationSnapshot;
    expect(fixtureValue(snapshot, fixture, "pan")).toBe(0.8);
  });
});
