import { describe, expect, it } from "vitest";
import {
  normalizePresetFamily,
  presetAddress,
  presetFamilyAcceptsAttribute,
  presetStorageKey,
} from "./presetFamilies";

describe("preset families", () => {
  it("migrates the legacy All name to Mixed", () => {
    expect(normalizePresetFamily("All")).toBe("Mixed");
    expect(normalizePresetFamily("Mixed")).toBe("Mixed");
  });

  it("limits typed families while Mixed accepts any attribute", () => {
    expect(presetFamilyAcceptsAttribute("Intensity", "head.intensity")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Color", "color.wheel.1")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Color", "color.cyan")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Position", "head.tilt")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Beam", "shutter")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Beam", "strobe")).toBe(true);
    expect(presetFamilyAcceptsAttribute("Color", "pan")).toBe(false);
    expect(presetFamilyAcceptsAttribute("Mixed", "custom.channel")).toBe(true);
  });

  it("uses numbers local to each preset family", () => {
    const colorOne = presetAddress("Color", 1);
    const positionOne = presetAddress("Position", 1);

    expect(presetStorageKey(colorOne)).toBe("2.1");
    expect(presetStorageKey(positionOne)).toBe("3.1");
    expect(colorOne.number).toBe(positionOne.number);
    expect(colorOne.family).not.toBe(positionOne.family);
  });
});
