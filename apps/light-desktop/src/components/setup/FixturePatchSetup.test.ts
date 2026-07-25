import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { compareFixtureIds, definitionModeChannels, definitionSplits, dmxGridSegments, draggedDmxStart, effectiveSplitPatches, formatFixturePatch, nextAvailableFixtureNumber, parseFixtureNumber, reconcileModePatchChanges, replaceSelectedSplitPatch, splitPatchSetError, unpatchFixtureChanges } from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const fixture = (fixture_number: number | null, fixture_id: string) => ({ fixture_number, fixture_id }) as PatchedFixture;

describe("Show Patch fixture ordering", () => {
  it("allocates positive fixture IDs from the requested start while skipping occupied IDs", () => {
    const used = new Set([1, 2, 101, 104]);
    expect(parseFixtureNumber("100")).toBe(100);
    expect(parseFixtureNumber("1.5")).toBeNull();
    expect(parseFixtureNumber("0")).toBeNull();
    expect(nextAvailableFixtureNumber(100, used)).toBe(100);
    expect(nextAvailableFixtureNumber(101, used)).toBe(102);
    expect(nextAvailableFixtureNumber(104, used)).toBe(105);
  });

  it("wraps DMX footprint outlines across grid rows and clamps dragged patches inside the universe", () => {
    expect(dmxGridSegments(15, 18)).toEqual([
      { row: 1, column: 15, length: 2 },
      { row: 2, column: 1, length: 2 },
    ]);
    expect(draggedDmxStart(50, 2, 4)).toBe(48);
    expect(draggedDmxStart(512, 0, 4)).toBe(509);
    expect(draggedDmxStart(1, 3, 4)).toBe(1);
  });

  it("sorts numbered fixtures by fixture ID and leaves unnumbered fixtures last", () => {
    const fixtures = [fixture(999, "rgb"), fixture(null, "z"), fixture(2, "two"), fixture(101, "one-oh-one"), fixture(null, "a")];
    expect(fixtures.sort(compareFixtureIds).map((item) => item.fixture_id)).toEqual(["two", "one-oh-one", "rgb", "a", "z"]);
  });

  it("projects a schema-v2 mode into independently patchable split assignments", () => {
    const profile = blankFixtureProfile();
    profile.modes[0].splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }];
    const definition = {
      schema_version: 2, id: profile.id, revision: 1, manufacturer: "Acme", device_type: "wash", name: "Split", model: "Split", mode: "Default", footprint: 4, heads: [], color_calibration: null, physical: {}, hazardous: false, direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {}, profile_id: profile.id, mode_id: profile.modes[0].id, profile_snapshot: profile,
    } as PatchedFixture["definition"];
    expect(definitionSplits(definition)).toEqual([{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }]);
    expect(definitionModeChannels(definition)).toEqual(profile.modes[0].channels);
    expect(effectiveSplitPatches(definition, [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: null, address: null }], 1, 101)).toEqual([{ split: 1, universe: 1, address: 101 }, { split: 3, universe: null, address: null }]);
    expect(formatFixturePatch({ fixture_id: "split", universe: 1, address: 101, definition, logical_heads: [], split_patches: [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: null, address: null }] })).toBe("S1 1.101 · S3 —");
    expect(splitPatchSetError(definition, [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 1, address: 103 }])).toContain("overlaps split 3");
    expect(splitPatchSetError(definition, [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: null, address: null }])).toBeNull();
    expect(splitPatchSetError(definition, [{ split: 1, universe: 1, address: 510 }, { split: 3, universe: null, address: null }])).toContain("512-slot universe");
  });

  it("edits only the SET-selected split and keeps the legacy primary address in sync", () => {
    const profile = blankFixtureProfile();
    profile.modes[0].splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }];
    const definition = {
      schema_version: 2, id: profile.id, revision: 1, manufacturer: "Acme", device_type: "wash", name: "Split", model: "Split", mode: "Default", footprint: 4, heads: [], color_calibration: null, physical: {}, hazardous: false, direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {}, profile_id: profile.id, mode_id: profile.modes[0].id, profile_snapshot: profile,
    } as PatchedFixture["definition"];
    const current = [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 2, address: 201 }];

    expect(replaceSelectedSplitPatch(definition, current, 1, 101, 3, { universe: 4, address: 401 })).toEqual({
      split_patches: [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 4, address: 401 }],
      universe: 1,
      address: 101,
    });
    expect(replaceSelectedSplitPatch(definition, current, 1, 101, 1, null)).toEqual({
      split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: 2, address: 201 }],
      universe: null,
      address: null,
    });
  });

  it("reconciles the main patch and every multi-patch when a mode changes split identities", () => {
    const oldProfile = blankFixtureProfile();
    oldProfile.modes[0].splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }];
    const oldDefinition = {
      schema_version: 2, id: oldProfile.id, revision: 1, manufacturer: "Acme", device_type: "wash", name: "Split", model: "Split", mode: "Old", footprint: 4, heads: [], color_calibration: null, physical: {}, hazardous: false, direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {}, profile_id: oldProfile.id, mode_id: oldProfile.modes[0].id, profile_snapshot: oldProfile,
    } as PatchedFixture["definition"];
    const nextProfile = structuredClone(oldProfile);
    nextProfile.modes[0].name = "New";
    nextProfile.modes[0].splits = [{ number: 1, footprint: 8 }, { number: 3, footprint: 6 }, { number: 5, footprint: 2 }];
    const nextDefinition = { ...oldDefinition, mode: "New", footprint: 8, profile_snapshot: nextProfile };
    const patched = {
      fixture_id: "split",
      definition: oldDefinition,
      universe: 1,
      address: 101,
      logical_heads: [],
      split_patches: [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 2, address: 201 }],
      multipatch: [{
        id: "mp-1", name: "Balcony", universe: 3, address: 301,
        split_patches: [{ split: 1, universe: 3, address: 301 }, { split: 3, universe: 4, address: 401 }],
        location: { x: 10, y: 20, z: 30 }, rotation: { x: 0, y: 90, z: 0 },
      }],
    } as PatchedFixture;

    expect(reconcileModePatchChanges(patched, nextDefinition)).toEqual({
      definition: nextDefinition,
      universe: 1,
      address: 101,
      split_patches: [
        { split: 1, universe: 1, address: 101 },
        { split: 3, universe: 2, address: 201 },
        { split: 5, universe: null, address: null },
      ],
      multipatch: [{
        id: "mp-1", name: "Balcony", universe: 3, address: 301,
        split_patches: [
          { split: 1, universe: 3, address: 301 },
          { split: 3, universe: 4, address: 401 },
          { split: 5, universe: null, address: null },
        ],
        location: { x: 10, y: 20, z: 30 }, rotation: { x: 0, y: 90, z: 0 },
      }],
    });
  });

  it("unpatches all schema-v2 splits and every physical multi-patch owner", () => {
    const profile = blankFixtureProfile();
    profile.modes[0].splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }];
    const definition = {
      schema_version: 2, id: profile.id, revision: 1, manufacturer: "Acme", device_type: "wash", name: "Split", model: "Split", mode: "Default", footprint: 4, heads: [], color_calibration: null, physical: {}, hazardous: false, direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {}, profile_id: profile.id, mode_id: profile.modes[0].id, profile_snapshot: profile,
    } as PatchedFixture["definition"];
    const patched = {
      fixture_id: "split", definition, universe: 1, address: 101, logical_heads: [],
      split_patches: [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 2, address: 201 }],
      multipatch: [{ id: "mp-1", name: "Balcony", universe: 3, address: 301, split_patches: [{ split: 1, universe: 3, address: 301 }, { split: 3, universe: 4, address: 401 }], location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 } }],
    } as PatchedFixture;

    expect(unpatchFixtureChanges(patched)).toEqual({
      universe: null,
      address: null,
      split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }],
      multipatch: [{ id: "mp-1", name: "Balcony", universe: null, address: null, split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }], location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 } }],
    });
  });
});
