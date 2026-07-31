import { describe, expect, it } from "vitest";
import { PLANNED_DEMO_FIXTURES } from "../../support/plannedDemoManifest";
import { createPlannedDemoPatchInputs } from "../../support/plannedDemoPatch";

const footprintByName: Record<string, number> = {
  "Robin DLS Profile": 36,
  "JBLED A7": 19,
  "RGBW LED": 5,
  "Dimmer Profile": 1,
  "Dimmer Fresnel": 1,
  "Sunstrip LED RGB 42206": 30,
  ACL: 1,
  Blinder: 2,
  Hazer: 2,
  Dimmer: 1,
};
const uniqueProfiles = [...new Map(PLANNED_DEMO_FIXTURES.map((fixture) => [
  `${fixture.profile.manufacturer}:${fixture.profile.name}`,
  fixture.profile,
])).values()];
const profiles = uniqueProfiles.map((profile, index) => ({
  id: `profile-${index}`,
  revision: 1,
  manufacturer: profile.manufacturer,
  name: profile.name,
  modes: [{
    id: `mode-${index}`,
    name: profile.mode,
    splits: [{ number: 1, footprint: footprintByName[profile.name] }],
  }],
})) as any;
const layers = Object.fromEntries(
  [
    "Stage & Venue", "Trusses", "Profile Stage", "Profile Audience",
    "Profile Auxilliary", "Wash Stage", "Wash Audience", "Wash Auxilliary",
    "LED PAR Stage", "LED PAR Audience", "LED PAR Auxilliary", "Front Lights",
    "Front Profiles", "ACLs & Blinder",
  ]
    .map((name) => [name, `layer-${name}`]),
);

describe("Plan 76 patch builder", () => {
  it("builds 262 stable controls and 301 physical instances with valid patches", () => {
    const built = createPlannedDemoPatchInputs(profiles, layers);
    expect(built.fixtureRecords).toBe(262);
    expect(built.physicalInstances).toBe(301);
    expect(built.lastUniverse).toBeGreaterThan(1);
    expect(new Set(built.fixtures.map((fixture) => fixture.fixture_id)).size).toBe(262);
    const occupied = new Set<string>();
    for (const fixture of built.fixtures) {
      for (const instance of [fixture, ...fixture.multipatch]) {
        for (const split of instance.split_patches) {
          if (split.universe == null) continue;
          const footprint = footprintByName[
            PLANNED_DEMO_FIXTURES.find((entry) => entry.number === fixture.fixture_number)!.profile.name
          ];
          for (let offset = 0; offset < footprint; offset++) {
            const slot = `${split.universe}:${split.address + offset}`;
            expect(occupied.has(slot)).toBe(false);
            occupied.add(slot);
            expect(split.address + offset).toBeLessThanOrEqual(512);
          }
        }
      }
    }
  });

  it("reserves universe 1 addresses 1-8 for the eight Front Lights", () => {
    const built = createPlannedDemoPatchInputs(profiles, layers);
    const fresnels = built.fixtures.filter((fixture) => fixture.fixture_number >= 1 && fixture.fixture_number <= 8);
    expect(fresnels.map((fixture) => [
      fixture.split_patches[0].universe,
      fixture.split_patches[0].address,
    ])).toEqual(Array.from({ length: 8 }, (_, index) => [1, index + 1]));
    expect(fresnels.slice(0, 4).map((fixture) => fixture.location.x)).toEqual([
      -4000, -3667, -3333, -3000,
    ]);
  });

  it("places four eight-lamp ACL fans on their literal trusses", () => {
    const built = createPlannedDemoPatchInputs(profiles, layers);
    const acls = built.fixtures.filter((fixture) => fixture.fixture_number >= 601 && fixture.fixture_number <= 604);
    expect(acls.every((fixture) => fixture.multipatch.length === 7)).toBe(true);
    expect(acls.map((fixture) => fixture.layer_id)).toEqual(
      Array.from({ length: 4 }, () => "layer-ACLs & Blinder"),
    );
    const front = [acls[3], ...acls[3].multipatch]
      .map((fixture) => fixture.location.x);
    expect(front.filter((x) => x < 0)).toHaveLength(4);
    expect(front.filter((x) => x > 0)).toHaveLength(4);
  });
});
