import { describe, expect, it } from "vitest";
import { plannedDemoGroupSpecs } from "../../support/plannedDemoGroups";

describe("Plan 76 Group manifest", () => {
  it("builds family, Show, Aux Show, odd/even, ACL, and utility memberships", () => {
    const groups = plannedDemoGroupSpecs();
    const byName = new Map(groups.map((group) => [group.name, group.fixtureNumbers]));
    expect(byName.get("Profile All")).toHaveLength(54);
    expect(byName.get("Wash All")).toHaveLength(46);
    expect(byName.get("LED All")).toHaveLength(132);
    expect(byName.get("Show")).toHaveLength(208);
    expect(byName.get("Aux Show")).toHaveLength(24);
    expect(byName.get("Show Profile Odd")).toHaveLength(25);
    expect(byName.get("Show Profile Even")).toHaveLength(25);
    expect(byName.get("All ACLs")).toEqual([601, 602, 603, 604]);
    expect(byName.get("Front Center")).toEqual([11, 12, 13]);
    expect(byName.get("Follow Spots")).toEqual([149, 150]);
    expect(groups.some((group) => group.name.includes("Aux Show") && /Odd|Even/.test(group.name))).toBe(false);
  });

  it("preserves ordered disjoint Show and Aux Show family memberships", () => {
    const byName = new Map(plannedDemoGroupSpecs().map((group) => [group.name, group.fixtureNumbers]));
    for (const family of ["Profile", "Wash", "LED"]) {
      const show = byName.get(`Show ${family}`)!;
      const aux = byName.get(`Aux Show ${family}`)!;
      expect(show.filter((fixture) => aux.includes(fixture))).toEqual([]);
      expect(new Set(show).size).toBe(show.length);
      expect(new Set(aux).size).toBe(aux.length);
    }
  });
});
