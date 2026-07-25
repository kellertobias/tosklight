import { describe, expect, it } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import {
  moveLampPositions,
  resolveLampPositions,
  returnHomeAssignments,
} from "./specialPosition";

const fixture = (id: string, pan: number, tilt: number): PatchedFixture => ({
  fixture_id: id,
  universe: 1,
  address: 1,
  logical_heads: [],
  definition: {
    schema_version: 1, id, revision: 1, manufacturer: "Test", device_type: "moving light",
    name: id, model: id, mode: "default", footprint: 2, color_calibration: null,
    physical: {}, hazardous: false, direct_control_protocols: [],
    signal_loss_policy: { type: "hold_last" }, safe_values: {},
    heads: [{ index: 0, name: "Main", shared: true, parameters: [
      { attribute: "pan", components: [], default: pan, virtual_dimmer: false, capabilities: [] },
      { attribute: "tilt", components: [], default: tilt, virtual_dimmer: false, capabilities: [] },
    ] }],
  },
});

const snapshot: VisualizationSnapshot = {
  revision: 1, generated_at: "2026-07-12T00:00:00Z", grand_master: 1, blackout: false,
  values: [
    { fixture_id: "lamp-a", attribute: "pan", value: { kind: "normalized", value: 0.2 } },
    { fixture_id: "lamp-a", attribute: "tilt", value: { kind: "normalized", value: 0.7 } },
    { fixture_id: "lamp-b", attribute: "pan", value: { kind: "normalized", value: 0.8 } },
  ],
};

describe("special position dialog", () => {
  it("starts every lamp at its own live position and fixture default", () => {
    const positions = resolveLampPositions(
      ["lamp-a", "lamp-b"],
      [fixture("lamp-a", 0.4, 0.5), fixture("lamp-b", 0.6, 0.3)],
      snapshot,
    );
    expect(positions.get("lamp-a")).toEqual({ pan: 0.2, tilt: 0.7 });
    expect(positions.get("lamp-b")).toEqual({ pan: 0.8, tilt: 0.3 });
  });

  it("moves each lamp relative to its own origin", () => {
    const positions = new Map([
      ["lamp-a", { pan: 0.2, tilt: 0.7 }],
      ["lamp-b", { pan: 0.8, tilt: 0.3 }],
    ]);
    moveLampPositions(positions, 1, -1, 0.1);
    expect(positions.get("lamp-a")).toEqual({ pan: 0.30000000000000004, tilt: 0.6 });
    expect(positions.get("lamp-b")).toEqual({ pan: 0.9, tilt: 0.19999999999999998 });
  });

  it("returns ordered selected heads to independent profile homes", () => {
    const parent = fixture("parent", 0.1, 0.2);
    parent.logical_heads = [
      { fixture_id: "head-a", head_index: 1 },
      { fixture_id: "head-b", head_index: 2 },
    ];
    parent.definition.heads = [
      parent.definition.heads[0],
      {
        index: 1,
        name: "A",
        shared: false,
        parameters: [
          { attribute: "pan", components: [], default: 0.25, virtual_dimmer: false, capabilities: [] },
          { attribute: "tilt", components: [], default: 0.75, virtual_dimmer: false, capabilities: [] },
        ],
      },
      {
        index: 2,
        name: "B",
        shared: false,
        parameters: [
          { attribute: "pan", components: [], default: 0.6, virtual_dimmer: false, capabilities: [] },
          { attribute: "intensity", components: [], default: 0, virtual_dimmer: false, capabilities: [] },
        ],
      },
    ];
    expect(returnHomeAssignments(["head-b", "head-a"], [parent])).toEqual([
      { fixtureId: "head-b", attribute: "pan", value: 0.6 },
      { fixtureId: "head-a", attribute: "pan", value: 0.25 },
      { fixtureId: "head-a", attribute: "tilt", value: 0.75 },
    ]);
  });

  it("falls back per missing home, skips non-position fixtures, and leaves empty selection alone", () => {
    const missingTiltHome = fixture("moving", 0.3, 0.4);
    delete (missingTiltHome.definition.heads[0].parameters[1] as { default?: number }).default;
    const dimmer = fixture("dimmer", 0, 0);
    dimmer.definition.heads[0].parameters = [
      { attribute: "intensity", components: [], default: 0, virtual_dimmer: false, capabilities: [] },
    ];
    expect(returnHomeAssignments(["moving", "dimmer"], [missingTiltHome, dimmer])).toEqual([
      { fixtureId: "moving", attribute: "pan", value: 0.3 },
      { fixtureId: "moving", attribute: "tilt", value: 0.5 },
    ]);
    expect(returnHomeAssignments([], [missingTiltHome])).toEqual([]);
  });
});
