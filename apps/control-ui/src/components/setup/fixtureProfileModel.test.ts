import { describe, expect, it } from "vitest";
import type { FixtureChannel, FixtureDefinition } from "../../api/types";
import {
  blankChannel,
  blankFixtureProfile,
  derivePrimarySlots,
  fixtureDefinitionFromProfileMode,
  fixtureDefinitionsFromProfiles,
  fixtureProfileFromDefinition,
  geometryTemplate,
  fixtureProfileFromDefinitions,
  maxRaw,
  mergeFixtureDefinitions,
  reconcileColorSystemHighlightDefaults,
  semanticHighlightRaw,
  semanticHighlightDefaultsForMode,
  validateProfile,
  xyyToXyz,
  xyzToXyy,
} from "./fixtureProfileModel";

function legacyDefinition(overrides: Partial<FixtureDefinition> = {}): FixtureDefinition {
  return {
    schema_version: 1,
    id: crypto.randomUUID(),
    revision: 1,
    manufacturer: "Generic",
    device_type: "dimmer",
    name: "Dimmer",
    model: "Dimmer",
    mode: "Standard",
    footprint: 1,
    heads: [],
    color_calibration: null,
    physical: {},
    model_asset: null,
    icon_asset: null,
    hazardous: false,
    direct_control_protocols: [],
    signal_loss_policy: { type: "hold_last" },
    safe_values: {},
    ...overrides,
  };
}

describe("fixture profile model", () => {
  it("accepts measured xyY without losing the normalized XYZ representation", () => {
    const xyz = xyyToXyz({ x: 0.3127, y: 0.329, luminance: 1 });
    const xyy = xyzToXyy(xyz);
    expect(xyy.x).toBeCloseTo(0.3127, 5);
    expect(xyy.y).toBeCloseTo(0.329, 5);
    expect(xyy.luminance).toBeCloseTo(1, 5);
  });
  it("starts with one Default mode, one editable main head, and revision zero", () => {
    const profile = blankFixtureProfile();
    expect(profile).toMatchObject({ schema_version: 2, revision: 0, modes: [{ name: "Default", heads: [{ name: "Main" }] }] });
    expect(profile.modes[0].heads[0].id).toBeTruthy();
  });

  it("derives full-and-white Highlight raw values for authored and imported channel semantics", () => {
    expect(semanticHighlightRaw("intensity", "u8", 0)).toBe(255);
    expect(semanticHighlightRaw("intensity", "u8", 0, true)).toBe(0);
    expect(semanticHighlightRaw("color.cyan", "u16", 123)).toBe(0);
    expect(semanticHighlightRaw("color.cyan", "u16", 123, true)).toBe(65_535);
    expect(semanticHighlightRaw("color.wheel.1", "u8", 7, false, [{ name: "Open / White", dmx_from: 10, dmx_to: 20 }])).toBe(15);
    expect(semanticHighlightRaw("color.wheel.1", "u8", 7, false, [{ name: "Red", dmx_from: 200, dmx_to: 255 }])).toBe(7);

    const attributes = ["intensity", "color.red", "color.green", "color.blue", "color.white", "color.cyan", "color.magenta", "color.yellow", "color.wheel.1", "pan"];
    const imported = fixtureProfileFromDefinition(legacyDefinition({
      footprint: attributes.length,
      heads: [{
        index: 0,
        name: "Main",
        shared: true,
        parameters: attributes.map((attribute, offset) => ({
          attribute,
          components: [{ offset, byte_order: "msb_first" as const }],
          default: attribute === "pan" ? 0.5 : attribute === "color.wheel.1" ? 7 / 255 : 0,
          virtual_dimmer: false,
          capabilities: attribute === "color.wheel.1" ? [{ name: "Open", dmx_from: 12, dmx_to: 18, preset_family: "color" }] : [],
        })),
      }],
    }));
    expect(Object.fromEntries(imported.modes[0].channels.map((channel) => [channel.attribute, channel.highlight_raw]))).toEqual({
      intensity: 255,
      "color.red": 255,
      "color.green": 255,
      "color.blue": 255,
      "color.white": 255,
      "color.cyan": 0,
      "color.magenta": 0,
      "color.yellow": 0,
      "color.wheel.1": 15,
      pan: 128,
    });
  });

  it("tracks new wheel and calibrated-color defaults only while Highlight raw is untouched", () => {
    const profile = blankFixtureProfile();
    const mode = profile.modes[0];
    const headId = mode.heads[0].id;
    const wheel = {
      ...blankChannel(mode),
      attribute: "color.wheel.1",
      default_raw: 7,
      highlight_raw: 7,
    };
    mode.channels = [wheel];
    const wheelSystem = [{
      head_id: headId,
      correction_matrix: [[1, 0, 0], [0, 1, 0], [0, 0, 1]] as [[number, number, number], [number, number, number], [number, number, number]],
      system: {
        type: "discrete_wheel" as const,
        channel_id: wheel.id,
        slots: [{ semantic_id: "color.open", label: "Open / White", dmx_from: 10, dmx_to: 20, measured_xyz: null }],
      },
    }];
    expect(reconcileColorSystemHighlightDefaults(mode, wheelSystem).channels[0].highlight_raw).toBe(15);
    expect(reconcileColorSystemHighlightDefaults({ ...mode, channels: [{ ...wheel, highlight_raw: 73 }] }, wheelSystem).channels[0].highlight_raw).toBe(73);

    const additiveMode = structuredClone(mode);
    additiveMode.splits[0].footprint = 3;
    additiveMode.channels = ["red", "green", "blue"].map((name) => ({
      ...blankChannel(additiveMode),
      attribute: `color.${name}`,
      highlight_raw: 255,
    }));
    additiveMode.channels[1].highlight_raw = 33;
    const additiveSystem = [{
      head_id: headId,
      correction_matrix: [[1, 0, 0], [0, 1, 0], [0, 0, 1]] as [[number, number, number], [number, number, number], [number, number, number]],
      system: {
        type: "additive" as const,
        emitters: additiveMode.channels.map((channel, index) => ({
          channel_id: channel.id,
          name: ["Red", "Green", "Blue"][index],
          xyz: [
            { x: 1, y: 0, z: 0 },
            { x: 0, y: 1, z: 0 },
            { x: 0, y: 0, z: 1 },
          ][index],
          maximum_level: 1,
          response_curve: 1,
          visible: true,
        })),
      },
    }];
    const expected = semanticHighlightDefaultsForMode({ ...additiveMode, channels: additiveMode.channels.map((channel, index) => index === 1 ? { ...channel, highlight_raw: 255 } : channel), color_systems: additiveSystem });
    const reconciled = reconcileColorSystemHighlightDefaults(additiveMode, additiveSystem);
    expect(reconciled.channels[0].highlight_raw).toBe(expected.get(reconciled.channels[0].id));
    expect(reconciled.channels[0].highlight_raw).toBeLessThan(255);
    expect(reconciled.channels[1].highlight_raw).toBe(33);
    expect(reconciled.channels[2].highlight_raw).toBe(expected.get(reconciled.channels[2].id));
  });

  it("derives primary slots around exact reserved component slots per split", () => {
    const profile = blankFixtureProfile();
    const mode = profile.modes[0];
    mode.splits = [{ number: 1, footprint: 6 }, { number: 2, footprint: 3 }];
    const splitTwoHead = { ...mode.heads[0], id: crypto.randomUUID(), name: "Remote", master_shared: false, split: 2 };
    mode.heads.push(splitTwoHead);
    const first = { ...blankChannel(mode, 1), resolution: "u16" as const, secondary_slots: [2] };
    const second = { ...blankChannel(mode, 1), resolution: "u24" as const, secondary_slots: [5, 6] };
    const third = blankChannel(mode, 1);
    const remote = { ...blankChannel(mode, 2), head_id: splitTwoHead.id };
    mode.channels = [first, second, third, remote];
    const result = derivePrimarySlots(mode);
    expect(result.errors).toEqual([]);
    expect([result.slots.get(first.id), result.slots.get(second.id), result.slots.get(third.id), result.slots.get(remote.id)]).toEqual([1, 3, 4, 1]);
  });

  it("keeps 32-bit raw integers exact and reports duplicate or invalid slots", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Acme";
    profile.name = "Exact";
    const mode = profile.modes[0];
    mode.splits[0].footprint = 4;
    const first: FixtureChannel = { ...blankChannel(mode), resolution: "u32", secondary_slots: [2, 3, 4], default_raw: maxRaw("u32"), highlight_raw: 0xffffffff };
    mode.channels = [first];
    expect(validateProfile(profile)).toEqual([]);
    first.secondary_slots = [2, 2, 4];
    expect(validateProfile(profile).join(" ")).toContain("duplicated");
    first.secondary_slots = [2, 3, 5];
    expect(validateProfile(profile).join(" ")).toContain("outside split");
  });

  it("creates all five geometry templates with their documented hierarchy and source layouts", () => {
    const heads = [crypto.randomUUID(), crypto.randomUUID()];
    const cases = [
      { template: "fixed", nodes: 1, layout: "point" },
      { template: "moving_head", nodes: 4, layout: "point" },
      { template: "bar", nodes: 1, layout: "strip" },
      { template: "matrix", nodes: 1, layout: "matrix" },
      { template: "shared_pan_multi_head", nodes: 4, layout: "point" },
    ] as const;
    for (const entry of cases) {
      const geometry = geometryTemplate(entry.template, heads);
      expect(geometry.nodes, entry.template).toHaveLength(entry.nodes);
      expect(geometry.emitters.map((emitter) => emitter.head_id), entry.template).toEqual(heads);
      expect(geometry.emitters.map((emitter) => emitter.layout.type), entry.template).toEqual([entry.layout, entry.layout]);
      const pan = geometry.nodes.find((node) => node.motion?.attribute === "pan");
      const tilts = geometry.nodes.filter((node) => node.motion?.attribute === "tilt");
      if (entry.template === "moving_head" || entry.template === "shared_pan_multi_head") {
        expect(pan, entry.template).toBeTruthy();
        expect(tilts, entry.template).toHaveLength(2);
        expect(tilts.every((node) => node.parent_id === pan?.id), entry.template).toBe(true);
      } else {
        expect(pan, entry.template).toBeUndefined();
        expect(tilts, entry.template).toEqual([]);
      }
    }
  });

  it("keeps every profile mode distinct while suppressing retained migrated legacy rows", () => {
    const profile = blankFixtureProfile();
    profile.revision = 3;
    profile.manufacturer = "Acme";
    profile.name = "Orbit";
    profile.short_name = "Orbit 500";
    profile.fixture_type = "wash mover";
    profile.modes[0].name = "Standard";
    profile.modes[0].splits[0].footprint = 12;
    profile.modes.push({ ...structuredClone(profile.modes[0]), id: crypto.randomUUID(), name: "Extended", splits: [{ number: 1, footprint: 24 }] });
    const migratedLegacy = legacyDefinition({
      id: profile.id,
      manufacturer: "ACME",
      name: "Orbit",
      model: "Orbit 500",
      device_type: "wash mover",
      mode: "Standard",
      footprint: 12,
    });
    const unrelated = legacyDefinition({ manufacturer: "Other" });
    const merged = mergeFixtureDefinitions([profile], [migratedLegacy, unrelated]);
    expect(merged).toHaveLength(3);
    expect(merged.map((definition) => definition.mode)).toEqual(["Standard", "Extended", "Standard"]);
    expect(new Set(merged.slice(0, 2).map((definition) => `${definition.profile_id}:${definition.mode_id}`)).size).toBe(2);
    expect(merged[0].profile_snapshot).toEqual(profile);
    expect(merged[2].id).toBe(unrelated.id);
  });

  it("shares one immutable library snapshot across large mode catalogs and clones editor drafts", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Generic";
    profile.name = "Many modes";
    const template = profile.modes[0];
    profile.modes = Array.from({ length: 128 }, (_, index) => ({
      ...structuredClone(template),
      id: crypto.randomUUID(),
      name: `Mode ${index + 1}`,
    }));

    const definitions = fixtureDefinitionsFromProfiles([profile]);
    expect(definitions).toHaveLength(128);
    expect(definitions.every((definition) => definition.profile_snapshot === profile)).toBe(true);

    const editorDraft = fixtureProfileFromDefinition(definitions[0]);
    expect(editorDraft).not.toBe(profile);
    editorDraft.name = "Edited draft";
    expect(profile.name).toBe("Many modes");
  });

  it("combines an ordered legacy/GDTF mode import into one unsaved atomic profile", () => {
    const standard = legacyDefinition({ manufacturer: "Acme", name: "Orbit", model: "Orbit 500", mode: "Standard", footprint: 8 });
    const extended = legacyDefinition({ manufacturer: "Acme", name: "Orbit", model: "Orbit 500", mode: "Extended", footprint: 16 });
    const profile = fixtureProfileFromDefinitions([standard, extended]);
    expect(profile).toMatchObject({ schema_version: 2, revision: 0, id: standard.id, manufacturer: "Acme", name: "Orbit", short_name: "Orbit 500" });
    expect(profile.modes.map((mode) => [mode.name, mode.splits[0].footprint])).toEqual([["Standard", 8], ["Extended", 16]]);
    expect(new Set(profile.modes.map((mode) => mode.id)).size).toBe(2);
  });

  it("keeps an additive head's color correction matrix in its portable fixture definition", () => {
    const profile = blankFixtureProfile();
    const mode = profile.modes[0];
    const correctionMatrix: [
      [number, number, number],
      [number, number, number],
      [number, number, number],
    ] = [
      [1.08, -0.03, -0.05],
      [0.01, 1.02, -0.03],
      [0, 0.06, 0.94],
    ];
    mode.color_systems = [{
      head_id: mode.heads[0].id,
      correction_matrix: correctionMatrix,
      system: { type: "additive", emitters: [] },
    }];

    expect(fixtureDefinitionFromProfileMode(profile, mode).color_calibration?.correction_matrix).toEqual(correctionMatrix);
  });
});
