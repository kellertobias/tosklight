import { describe, expect, it } from "vitest";
import type { CueList, PatchedFixture, ProgrammerState } from "../api/types";
import {
  activeProgrammerFixtureIds,
  compareFixtureIds,
  cueListFixtureIds,
  fixtureIsIncluded,
} from "./fixtureSheetFilters";

const groups = [{
  id: "front",
  body: { fixtures: ["fixture-2", "head-3"] },
}];

describe("fixture sheet filters", () => {
  it("includes direct and group programmer fixtures", () => {
    const programmer = {
      values: [{ fixture_id: "fixture-1" }],
      group_values: { front: { intensity: { value: {}, changed_at: "now" } } },
    } as unknown as ProgrammerState;
    expect([...activeProgrammerFixtureIds(programmer, groups)].sort()).toEqual([
      "fixture-1",
      "fixture-2",
      "head-3",
    ]);
  });

  it("includes fixtures used directly or through groups anywhere in a cue list", () => {
    const cueList = {
      cues: [{
        changes: [{ fixture_id: "fixture-1" }],
        group_changes: [{ group_id: "front" }],
      }],
    } as unknown as CueList;
    expect([...cueListFixtureIds(cueList, groups)!].sort()).toEqual([
      "fixture-1",
      "fixture-2",
      "head-3",
    ]);
  });

  it("matches logical heads and orders missing fixture numbers last", () => {
    const fixture = {
      fixture_id: "fixture-3",
      logical_heads: [{ fixture_id: "head-3", head_index: 0 }],
    } as PatchedFixture;
    expect(fixtureIsIncluded(fixture, new Set(["head-3"]))).toBe(true);
    expect(compareFixtureIds(
      { fixture_id: "missing", fixture_number: null } as PatchedFixture,
      { fixture_id: "numbered", fixture_number: 12 } as PatchedFixture,
    )).toBeGreaterThan(0);
    expect(compareFixtureIds(
      { fixture_id: "visual", fixture_number: null, virtual_fixture_number: 1 } as PatchedFixture,
      { fixture_id: "numbered", fixture_number: 1 } as PatchedFixture,
    )).toBeLessThan(0);
  });
});
