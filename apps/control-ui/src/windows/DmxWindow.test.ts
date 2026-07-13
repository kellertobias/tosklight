import { describe, expect, it } from "vitest";
import { dmxChannelsPerRow } from "./DmxWindow";

describe("responsive DMX grid", () => {
  it("uses readable desktop rows instead of compressing all 64 values", () => {
    expect(dmxChannelsPerRow(900, "small")).toBe(32);
    expect(dmxChannelsPerRow(1400, "small")).toBe(64);
  });

  it("allocates larger cells for touch mode", () => {
    expect(dmxChannelsPerRow(900, "large")).toBe(32);
    expect(dmxChannelsPerRow(600, "large")).toBe(16);
  });
});
