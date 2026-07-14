import { describe, expect, it } from "vitest";
import { dmxChannelsPerRow } from "./DmxWindow";

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
