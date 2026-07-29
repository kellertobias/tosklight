import { describe, expect, it } from "vitest";
import {
  PLANNED_DEMO_CONTROL_FIXTURES,
  PLANNED_DEMO_FIRST_LEVEL_GROUPS,
  PLANNED_DEMO_FIXTURES,
  PLANNED_DEMO_PHYSICAL_INSTANCES,
  plannedDemoFamilyNumbers,
  plannedDemoRoleNumbers,
} from "../../support/plannedDemoManifest";

describe("Plan 76 demo manifest", () => {
  it("owns the exact controllable and physical inventory", () => {
    expect(PLANNED_DEMO_FIXTURES).toHaveLength(PLANNED_DEMO_CONTROL_FIXTURES);
    expect(new Set(PLANNED_DEMO_FIXTURES.map((fixture) => fixture.number)).size).toBe(262);
    expect(PLANNED_DEMO_FIXTURES.reduce(
      (count, fixture) => count + 1 + fixture.multipatches,
      0,
    )).toBe(PLANNED_DEMO_PHYSICAL_INSTANCES);
    expect(PLANNED_DEMO_FIXTURES.some((fixture) =>
      fixture.profile.name.toLowerCase().includes("beam"))).toBe(false);
  });

  it("reconciles every moving-family location group", () => {
    const expected = {
      profile: { stage: 28, audience: 22, aux: 4, all: 54 },
      wash: { stage: 26, audience: 16, aux: 4, all: 46 },
      led: { stage: 16, audience: 100, aux: 16, all: 132 },
    } as const;
    for (const [family, counts] of Object.entries(expected)) {
      expect(plannedDemoFamilyNumbers(family as "profile" | "wash" | "led", "stage")).toHaveLength(counts.stage);
      expect(plannedDemoFamilyNumbers(family as "profile" | "wash" | "led", "audience")).toHaveLength(counts.audience);
      expect(plannedDemoFamilyNumbers(family as "profile" | "wash" | "led", "aux")).toHaveLength(counts.aux);
      expect(plannedDemoFamilyNumbers(family as "profile" | "wash" | "led")).toHaveLength(counts.all);
    }
    expect(PLANNED_DEMO_FIRST_LEVEL_GROUPS).toHaveLength(12);
  });

  it("defines four eight-lamp ACL controls and the approved Front Split composition", () => {
    for (const role of ["ACL 1", "ACL 2", "ACL 3", "ACL 4"])
      expect(plannedDemoRoleNumbers(role)).toHaveLength(1);
    const acls = PLANNED_DEMO_FIXTURES.filter((fixture) => fixture.roles.includes("All ACLs"));
    expect(acls.map((fixture) => fixture.name)).toEqual([
      "Back Centre ACL", "Back Split ACL", "Mid Split ACL", "Front Split ACL",
    ]);
    expect(acls.every((fixture) => fixture.multipatches === 7)).toBe(true);
    expect(plannedDemoRoleNumbers("Follow Spots")).toEqual([149, 150]);
    expect(plannedDemoRoleNumbers("House Lights")).toEqual([901]);
  });
});
