import { describe, expect, it } from "vitest";
import { plannedDemoGroupSpecs } from "../../support/plannedDemoGroups";

describe("Plan 76 Group manifest", () => {
  it("builds family, Show, Aux Show, odd/even, ACL, and utility memberships", () => {
    const groups = plannedDemoGroupSpecs();
    const byName = new Map(groups.map((group) => [group.name, group.fixtureNumbers]));
		expect(byName.get("Beam Stage")).toHaveLength(28);
		expect(byName.get("Beam Audience")).toHaveLength(22);
		expect(byName.get("Beam Auxiliary")).toHaveLength(4);
		expect(byName.get("Beam Show")).toHaveLength(50);
		expect(byName.get("Beam Show Odd")).toHaveLength(25);
		expect(byName.get("Beam Show Even")).toHaveLength(25);
    expect(byName.get("All ACLs")).toEqual([601, 602, 603, 604]);
		expect(byName.get("Strobe")).toEqual([]);
		expect(byName.get("Floor Spots")).toEqual([]);
    expect(byName.get("Follow Spots")).toEqual([149, 150]);
		expect(groups.map((group) => Number(group.id))).toEqual(
			Array.from({ length: 35 }, (_, index) => index + 1),
		);
  });

  it("preserves ordered disjoint Show and Aux Show family memberships", () => {
    const byName = new Map(plannedDemoGroupSpecs().map((group) => [group.name, group.fixtureNumbers]));
		for (const family of ["Beam", "Wash", "LED"]) {
			const show = byName.get(`${family} Show`)!;
			const aux = byName.get(`${family} Auxiliary Show`)!;
      expect(show.filter((fixture) => aux.includes(fixture))).toEqual([]);
      expect(new Set(show).size).toBe(show.length);
      expect(new Set(aux).size).toBe(aux.length);
    }
  });
});
