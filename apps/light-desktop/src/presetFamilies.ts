export const PRESET_FAMILIES = ["Mixed", "Intensity", "Color", "Position", "Beam"] as const;

export type PresetFamily = (typeof PRESET_FAMILIES)[number];

export const PRESET_FAMILY_TYPE: Record<PresetFamily, number> = {
  Mixed: 0,
  Intensity: 1,
  Color: 2,
  Position: 3,
  Beam: 4,
};

export interface PresetAddress {
  family: PresetFamily;
  number: number;
}

export function presetAddress(family: PresetFamily, number: number): PresetAddress {
  if (!Number.isInteger(number) || number < 1) throw new Error("Preset numbers start at 1");
  return { family, number };
}

/** Persistence/operator address only; the Preset's domain ID is its family-local `number`. */
export function presetStorageKey(address: PresetAddress): string {
  return `${PRESET_FAMILY_TYPE[address.family]}.${address.number}`;
}

export function normalizePresetFamily(value: unknown, fallback: PresetFamily = "Mixed"): PresetFamily {
  if (value === "All" || value === "all") return "Mixed";
  return PRESET_FAMILIES.includes(value as PresetFamily) ? value as PresetFamily : fallback;
}

export function presetFamilyAcceptsAttribute(family: PresetFamily, attribute: string): boolean {
  if (family === "Mixed") return true;
  const normalized = attribute.toLowerCase();
  const parts = normalized.split(".");
  if (family === "Intensity") return parts.includes("intensity") || parts.includes("dimmer");
  if (family === "Color") return normalized === "color" || parts.includes("color");
  if (family === "Position") return parts.includes("pan") || parts.includes("tilt") || parts.includes("position");
  return parts.some((part) => ["beam", "focus", "zoom", "iris", "gobo", "prism", "frost", "shaper", "shutter", "strobe"].includes(part));
}
