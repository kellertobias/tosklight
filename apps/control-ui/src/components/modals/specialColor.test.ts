import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import {
  colorProgrammerAssignments,
  interpolatePickerRange,
} from "./specialColor";

function fixture(
  fixtureId: string,
  attributes: string[],
  logicalHead?: { fixtureId: string; index: number },
): PatchedFixture {
  return {
    fixture_id: fixtureId,
    universe: 1,
    address: 1,
    logical_heads: logicalHead
      ? [{ fixture_id: logicalHead.fixtureId, head_index: logicalHead.index }]
      : [],
    definition: {
      schema_version: 1,
      id: fixtureId,
      revision: 1,
      manufacturer: "Test",
      device_type: "light",
      name: fixtureId,
      model: fixtureId,
      mode: "default",
      footprint: attributes.length,
      color_calibration: null,
      physical: {},
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
      heads: [{
        index: logicalHead?.index ?? 0,
        name: "Main",
        shared: !logicalHead,
        parameters: attributes.map((attribute) => ({
          attribute,
          components: [],
          default: 0,
          virtual_dimmer: false,
          capabilities: [],
        })),
      }],
    },
  };
}

describe("Color special dialog range", () => {
  it("uses the visible straight picker line and gives one fixture the release endpoint", () => {
    const start = { hue: 0.9, saturation: 0.2, brightness: 0.7 };
    const end = { hue: 0.1, saturation: 0.8, brightness: 0.6 };
    expect(interpolatePickerRange(3, start, end)).toEqual([
      { hue: 0.9, saturation: 0.2, brightness: 0.6 },
      { hue: 0.5, saturation: 0.5, brightness: 0.6 },
      { hue: 0.1, saturation: 0.8, brightness: 0.6 },
    ]);
    expect(interpolatePickerRange(1, start, end)).toEqual([end]);
  });

  it("preserves selection order and safely resolves RGB, CMY, logical, and unsupported targets", () => {
    const rgb = fixture("rgb", ["color.red", "color.green", "color.blue"]);
    const cmy = fixture("cmy-parent", ["color.cyan", "color.magenta", "color.yellow"], {
      fixtureId: "cmy-head",
      index: 2,
    });
    const dimmer = fixture("dimmer", ["intensity"]);
    const colors = interpolatePickerRange(3,
      { hue: 0, saturation: 1, brightness: 1 },
      { hue: 2 / 3, saturation: 1, brightness: 1 },
    );
    const assignments = colorProgrammerAssignments(
      ["cmy-head", "dimmer", "rgb"],
      [rgb, cmy, dimmer],
      colors,
    );
    expect(assignments.slice(0, 3)).toEqual([
      { fixtureId: "cmy-head", attribute: "color.cyan", value: 0 },
      { fixtureId: "cmy-head", attribute: "color.magenta", value: 1 },
      { fixtureId: "cmy-head", attribute: "color.yellow", value: 1 },
    ]);
    expect(assignments.slice(3)).toEqual([
      { fixtureId: "rgb", attribute: "color.red", value: 0 },
      { fixtureId: "rgb", attribute: "color.green", value: 0 },
      { fixtureId: "rgb", attribute: "color.blue", value: 1 },
    ]);

    const reversed = colorProgrammerAssignments(
      ["rgb", "dimmer", "cmy-head"],
      [rgb, cmy, dimmer],
      colors,
    );
    expect(reversed.slice(0, 3).map(({ fixtureId, value }) => ({ fixtureId, value }))).toEqual([
      { fixtureId: "rgb", value: 1 },
      { fixtureId: "rgb", value: 0 },
      { fixtureId: "rgb", value: 0 },
    ]);
    expect(reversed.slice(3).map(({ fixtureId, value }) => ({ fixtureId, value }))).toEqual([
      { fixtureId: "cmy-head", value: 1 },
      { fixtureId: "cmy-head", value: 1 },
      { fixtureId: "cmy-head", value: 0 },
    ]);
  });
});
