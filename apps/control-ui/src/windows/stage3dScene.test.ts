import { describe, expect, it } from "vitest";
import type { VisualizationSnapshot } from "../api/types";
import { cueVisualization, migrateStagePosition } from "./stage3dScene";

describe("3D stage state", () => {
  it("migrates legacy percentage positions into the meter-based stage", () => {
    expect(migrateStagePosition({ x: 50, y: 25, rotation: 90 }, 0)).toEqual({
      x: 0, y: 2, z: 5, rotationX: 0, rotationY: 0, rotationZ: 90,
    });
  });

  it("tracks cue values and explicit releases for thumbnails", () => {
    const base: VisualizationSnapshot = { revision: 1, generated_at: "", grand_master: .5, blackout: true, values: [] };
    const first = cueVisualization(base, [{ fixture_id: "one", attribute: "intensity", value: { kind: "normalized", value: .8 } }]);
    expect(first.blackout).toBe(false);
    expect(first.grand_master).toBe(1);
    expect(first.values).toHaveLength(1);
    const released = cueVisualization(first, [{ fixture_id: "one", attribute: "intensity", value: null }]);
    expect(released.values).toHaveLength(0);
  });
});
