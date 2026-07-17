import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import { maxRaw } from "./fixtureProfileModel";

export const incrementFixtureName = (base: string, index: number) => { const match = base.match(/^(.*?)(\d+)(\.\d+)?$/); if (!match) return index ? `${base || "Fixture"} ${index + 1}` : base || "Fixture"; return `${match[1]}${Number(match[2]) + index}${match[3] ?? ""}`; };
function splitFootprints(definition: FixtureDefinition) {
  const profile = definition.profile_snapshot;
  const mode = profile?.modes.find((candidate) => candidate.id === definition.mode_id) ?? profile?.modes.find((candidate) => candidate.name === definition.mode) ?? profile?.modes[0];
  return new Map((mode?.splits ?? [{ number: 1, footprint: definition.footprint }]).map((split) => [split.number, split.footprint]));
}
function rangesFor(definition: FixtureDefinition, splitPatches: PatchedFixture["split_patches"], universe: number | null, address: number | null) {
  const footprints = splitFootprints(definition);
  if (splitPatches?.length) return splitPatches.flatMap((patch) => patch.universe && patch.address ? [{ universe: patch.universe, start: patch.address, end: patch.address + (footprints.get(patch.split) ?? definition.footprint) - 1, split: patch.split }] : []);
  return universe && address ? [{ universe, start: address, end: address + definition.footprint - 1, split: 1 }] : [];
}
export function fixtureRange(fixture: PatchedFixture) { return rangesFor(fixture.definition, fixture.split_patches, fixture.universe, fixture.address)[0] ?? null; }
export function fixtureRanges(fixture: PatchedFixture) { return [...rangesFor(fixture.definition, fixture.split_patches, fixture.universe, fixture.address), ...(fixture.multipatch ?? []).flatMap((instance) => rangesFor(fixture.definition, instance.split_patches, instance.universe, instance.address))]; }
export function conflicts(fixtures: PatchedFixture[], universe: number, address: number, footprint: number, except?: string) { const end = address + footprint - 1; return fixtures.filter((fixture) => fixture.fixture_id !== except && fixtureRanges(fixture).some((range) => range.universe === universe && range.start <= end && range.end >= address)); }
export function firstFreeAddress(fixtures: PatchedFixture[], universe: number, footprint: number, from = 1) { for (let address = Math.max(1, from); address + footprint - 1 <= 512; address++) if (!conflicts(fixtures, universe, address, footprint).length) return address; return null; }
export function groupFixtureFamilies(definitions: FixtureDefinition[]) { const grouped = new Map<string, { key: string; manufacturer: string; name: string; deviceType: string; modes: FixtureDefinition[] }>(); for (const definition of definitions) { const key = `${definition.manufacturer}\0${definition.model || definition.name}`; const family = grouped.get(key) ?? { key, manufacturer: definition.manufacturer, name: definition.name || definition.model, deviceType: definition.device_type || "other", modes: [] }; family.modes.push(definition); grouped.set(key, family); } return [...grouped.values()].map((family) => ({ ...family, modes: family.modes.sort((a,b) => a.mode.localeCompare(b.mode)) })).sort((a,b) => `${a.manufacturer} ${a.name}`.localeCompare(`${b.manufacturer} ${b.name}`)); }

export function compatibleHighlightOverrides(
  definition: FixtureDefinition,
  overrides: Record<string, number> | undefined,
) {
  const profile = definition.profile_snapshot;
  const mode = profile?.modes.find((candidate) => candidate.id === definition.mode_id)
    ?? profile?.modes.find((candidate) => candidate.name === definition.mode)
    ?? profile?.modes[0];
  const channels = new Map((mode?.channels ?? []).map((channel) => [channel.id, channel]));
  return Object.fromEntries(Object.entries(overrides ?? {}).filter(([channelId, raw]) => {
    const channel = channels.get(channelId);
    return channel != null && Number.isInteger(raw) && raw >= 0 && raw <= maxRaw(channel.resolution);
  }));
}
