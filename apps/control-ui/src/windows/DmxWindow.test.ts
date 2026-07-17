import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../api/types";
import { blankChannel, blankFixtureProfile, blankHead, fixtureDefinitionFromProfileMode } from "../components/setup/fixtureProfileModel";
import { dmxChannelsPerRow, fixtureChannelAt, fixtureDmxPatchBindings } from "./DmxWindow";

describe("responsive DMX grid", () => {
  it("uses every channel that fits rather than fixed power-of-two rows", () => {
    expect(dmxChannelsPerRow(900, "small")).toBe(69);
    expect(dmxChannelsPerRow(1400, "small")).toBe(110);
    expect(dmxChannelsPerRow(710, "small")).toBe(53);
  });

  it("allocates larger cells for touch mode", () => {
    expect(dmxChannelsPerRow(900, "large")).toBe(18);
    expect(dmxChannelsPerRow(600, "large")).toBe(11);
  });
});

describe("DMX fixture channel details", () => {
  const fixture = {
    fixture_id: "fixture-uuid",
    fixture_number: 21,
    name: "Stage right profile",
    universe: 2,
    address: 101,
    definition: {
      footprint: 4,
      heads: [{ parameters: [
        { attribute: "intensity", components: [{ offset: 0 }] },
        { attribute: "pan", components: [{ offset: 1 }, { offset: 2 }] },
      ] }],
    },
    multipatch: [{ universe: 3, address: 201 }],
  } as unknown as PatchedFixture;

  it("reports fixture-relative channel, attribute, and component", () => {
    expect(fixtureChannelAt([fixture], 2, 102)).toMatchObject({ fixtureChannel: 2, attribute: "pan", component: "coarse" });
    expect(fixtureChannelAt([fixture], 2, 103)).toMatchObject({ fixtureChannel: 3, attribute: "pan", component: "fine" });
  });

  it("uses the selected multipatch address and returns null for empty slots", () => {
    expect(fixtureChannelAt([fixture], 3, 202)).toMatchObject({ fixtureChannel: 2, attribute: "pan" });
    expect(fixtureChannelAt([fixture], 2, 200)).toBeNull();
  });

  it("derives every schema-v2 split and multi-patch range with its physical owner", () => {
    const profile = blankFixtureProfile();
    profile.id = "split-profile";
    profile.revision = 4;
    const mode = profile.modes[0];
    const main = mode.heads[0];
    const remote = blankHead(1, 3);
    mode.splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 6 }];
    mode.heads = [{ ...main, split: 1 }, remote];
    mode.channels = [
      { ...blankChannel(mode, 1), id: "intensity", head_id: main.id, attribute: "intensity" },
      { ...blankChannel(mode, 3), id: "pan", head_id: remote.id, attribute: "pan", resolution: "u16", secondary_slots: [4] },
    ];
    const definition = fixtureDefinitionFromProfileMode(profile, mode);
    const splitFixture = {
      ...fixture,
      definition,
      universe: 1,
      address: 101,
      split_patches: [
        { split: 1, universe: 1, address: 101 },
        { split: 3, universe: 2, address: 201 },
      ],
      multipatch: [{
        id: "balcony",
        name: "Balcony duplicate",
        universe: 3,
        address: 301,
        split_patches: [
          { split: 1, universe: 3, address: 301 },
          { split: 3, universe: 4, address: 401 },
        ],
        location: { x: 0, y: 0, z: 0 },
        rotation: { x: 0, y: 0, z: 0 },
      }],
    } as PatchedFixture;

    expect(fixtureDmxPatchBindings(splitFixture)).toEqual([
      { split: 1, footprint: 4, universe: 1, address: 101, owner: { kind: "fixture", id: "fixture-uuid", name: "Fixture patch" } },
      { split: 3, footprint: 6, universe: 2, address: 201, owner: { kind: "fixture", id: "fixture-uuid", name: "Fixture patch" } },
      { split: 1, footprint: 4, universe: 3, address: 301, owner: { kind: "multipatch", id: "balcony", name: "Balcony duplicate" } },
      { split: 3, footprint: 6, universe: 4, address: 401, owner: { kind: "multipatch", id: "balcony", name: "Balcony duplicate" } },
    ]);
    expect(fixtureChannelAt([splitFixture], 2, 204)).toMatchObject({
      fixtureChannel: 4,
      split: 3,
      splitFootprint: 6,
      attribute: "pan",
      component: "fine",
      patchOwner: { kind: "fixture", id: "fixture-uuid" },
      patchRange: { universe: 2, start: 201, end: 206 },
    });
    expect(fixtureChannelAt([splitFixture], 4, 401)).toMatchObject({
      fixtureChannel: 1,
      split: 3,
      attribute: "pan",
      component: "coarse",
      patchOwner: { kind: "multipatch", id: "balcony", name: "Balcony duplicate" },
      patchRange: { universe: 4, start: 401, end: 406 },
    });
    expect(fixtureChannelAt([splitFixture], 3, 301)).toMatchObject({ split: 1, attribute: "intensity" });
    expect(fixtureChannelAt([splitFixture], 2, 200)).toBeNull();
  });
});
