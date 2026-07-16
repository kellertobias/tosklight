import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../api/types";
import { dmxChannelsPerRow, fixtureChannelAt } from "./DmxWindow";

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
});
