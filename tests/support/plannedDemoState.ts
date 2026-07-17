import fs from "node:fs/promises";
import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import type { FixtureDefinition, FixtureProfile, PatchedFixture } from "../../apps/control-ui/src/api/types";
import { fixtureDefinitionsFromProfiles } from "../../apps/control-ui/src/components/setup/fixtureProfileModel";

interface VersionedObject<T = Record<string, any>> {
  id: string;
  revision: number;
  body: T;
}

export interface PlannedDemoRig {
  fixtures: Record<number, PatchedFixture>;
  profileTargets: string[];
  washTargets: string[];
  floorTargets: string[];
  stripTargets: string[];
}

interface FixtureInput {
  number: number;
  name: string;
  definition: FixtureDefinition;
  layerId: string;
  universe?: number | null;
  address?: number | null;
  x?: number;
  y?: number;
  z?: number;
  rotation?: { x: number; y: number; z: number };
  multipatch?: Array<{
    name: string;
    universe?: number | null;
    address?: number | null;
    x: number;
    y: number;
    z: number;
    rotation?: { x: number; y: number; z: number };
  }>;
}

export async function seedPlannedDemoPatch(
  api: ApiDriver,
  showId: string,
  layers: Record<string, string>,
): Promise<PlannedDemoRig> {
  const legacyLibrary = await api.request<FixtureDefinition[]>("GET", "/api/v1/fixture-library", undefined, false);
  let profiles = await api.request<FixtureProfile[]>("GET", "/api/v1/fixture-profiles", undefined, false);
  const packages = [
    ["Venue", "Stage Element 2 × 1 m", "venue--stage-element-2-1-m.toskfixture"],
    ["Venue", "Four-Point Truss", "venue--four-point-truss.toskfixture"],
    ["Venue", "One-Point Truss / Pipe", "venue--one-point-truss-pipe.toskfixture"],
    ["Venue", "Curtain 2 m", "venue--curtain-2-m.toskfixture"],
    ["Generic", "Dimmer Fresnel", "generic--dimmer-fresnel.toskfixture"],
    ["Generic", "Dimmer", "generic--dimmer.toskfixture"],
    ["Generic", "Dimmer PAR Can", "generic--dimmer-par-can.toskfixture"],
    ["ROBE", "Robin DLS Profile", "robe--robin-dls-profile.toskfixture"],
    ["JB-Lighting", "JBLED A7", "jb-lighting--jbled-a7.toskfixture"],
    ["Showtec", "Sunstrip LED RGB 42206", "showtec--sunstrip-led-rgb-42206.toskfixture"],
    ["Generic", "RGBW LED", "generic--rgbw-led.toskfixture"],
    ["Generic", "Blinder", "generic--blinder.toskfixture"],
    ["Generic", "Hazer", "generic--hazer.toskfixture"],
  ] as const;
  for (const [manufacturer, name, archive] of packages) {
    if (profiles.some((candidate) => candidate.manufacturer === manufacturer && candidate.name === name)) continue;
    const bytes = await fs.readFile(new URL(`../../fixture-library/${archive}`, import.meta.url));
    const response = await fetch(`${api.baseUrl}/api/v1/fixture-packages/import`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${api.session?.token}`,
        "content-type": "application/vnd.tosklight.fixture+zip",
      },
      body: bytes,
    });
    if (!response.ok) throw new Error(`Importing ${archive} returned ${response.status}: ${await response.text()}`);
    profiles = await api.request<FixtureProfile[]>("GET", "/api/v1/fixture-profiles", undefined, false);
  }
  const library = [...legacyLibrary, ...fixtureDefinitionsFromProfiles(profiles)];
  const definition = (manufacturer: string, name: string, mode: string) => {
    const found = library.find((candidate) => candidate.manufacturer === manufacturer && candidate.name === name && candidate.mode === mode);
    if (!found) {
      const available = library
        .filter((candidate) => candidate.manufacturer === manufacturer)
        .map((candidate) => `${candidate.name} / ${candidate.mode}`)
        .join(", ");
      throw new Error(`Missing demo fixture definition: ${manufacturer} / ${name} / ${mode}. Available: ${available || "none"}`);
    }
    return found;
  };
  const stage = definition("Venue", "Stage Element 2 × 1 m", "50 cm");
  const truss = definition("Venue", "Four-Point Truss", "2 m");
  const pipe = definition("Venue", "One-Point Truss / Pipe", "2.5 m");
  const curtain = definition("Venue", "Curtain 2 m", "5 m");
  const fresnel = definition("Generic", "Dimmer Fresnel", "8-bit");
  const dimmer = definition("Generic", "Dimmer", "8-bit");
  const acl = definition("Generic", "Dimmer PAR Can", "8-bit");
  const profile = definition("ROBE", "Robin DLS Profile", "Mode 3");
  const wash = definition("JB-Lighting", "JBLED A7", "Standard RGB 16 Bit (S16)");
  const strip = definition("Showtec", "Sunstrip LED RGB 42206", "30 Channel");
  const floor = definition("Generic", "RGBW LED", "DRGBW 8-bit dimmer first");
  const blinder = definition("Generic", "Blinder", "Two channel, four blind");
  const hazer = definition("Generic", "Hazer", "Fan, Fog");
  const inputs: FixtureInput[] = [];

  let venueNumber = 10_001;
  for (const [row, y] of [-1.5, -.5, .5, 1.5].entries()) {
    for (const [column, x] of [-3, -1, 1, 3].entries()) {
      inputs.push({ number: venueNumber++, name: `Stage ${row + 1}.${column + 1}`, definition: stage, layerId: layers.Stage, x, y, z: 0 });
    }
  }
  for (const [name, y] of [["Back Truss", 4], ["Mid Truss", 0], ["Front Truss", -3]] as const) {
    inputs.push({
      number: venueNumber++, name, definition: truss, layerId: layers.Stage, x: -3, y, z: 4.15,
      multipatch: [-1, 1, 3].map((x, index) => ({ name: `${name} ${index + 2}`, x, y, z: 4.15 })),
    });
  }
  [-1.5, -.5, .5, 1.5].forEach((x, index) => inputs.push({ number: venueNumber++, name: `Pipe ${index + 1}`, definition: pipe, layerId: layers.Stage, x, y: 4.2, z: 3.05 }));
  [-2, 2].forEach((x, index) => inputs.push({ number: venueNumber++, name: `Curtain ${index + 1}`, definition: curtain, layerId: layers.Stage, x, y: 4.3, z: 2.5 }));

  [-3.8, -3.533, -3.267, -3].forEach((x, index) => inputs.push({ number: index + 1, name: `Front Left ${index + 1}`, definition: fresnel, layerId: layers["Front Truss"], universe: 2, address: index + 1, x, y: -3, z: 4, rotation: { x: -90, y: 0, z: 0 } }));
  [3, 3.267, 3.533, 3.8].forEach((x, index) => inputs.push({ number: index + 5, name: `Front Right ${index + 1}`, definition: fresnel, layerId: layers["Front Truss"], universe: 2, address: index + 7, x, y: -3, z: 4, rotation: { x: -90, y: 0, z: 0 } }));
  inputs.push({
    number: 99, name: "House Light", definition: dimmer, layerId: layers["House Lights"], universe: 2, address: 13,
    multipatch: [14, 15, 16].map((address) => ({ name: `House Light ${address - 12}`, universe: 2, address, x: 0, y: -5 + address - 14, z: 4 })),
  });
  inputs.push({
    number: 98, name: "House Mood", definition: dimmer, layerId: layers["House Lights"], universe: 2, address: 17,
    multipatch: [18, 19, 20, 21, 22, 23, 24].map((address) => ({ name: `House Mood ${address - 16}`, universe: 2, address, x: -3 + address - 18, y: -4, z: 3 })),
  });
  const fan = (outside: boolean) => Array.from({ length: 7 }, (_, index) => {
    const centered = index - 3;
    return { name: `${outside ? "ACL Out" : "ACL In"} ${index + 2}`, x: outside ? centered * 1.25 : centered * .35, y: 4, z: 4.05, rotation: { x: -20, y: outside ? centered * -9 : centered * 9, z: 0 } };
  });
  inputs.push({ number: 81, name: "ACL In", definition: acl, layerId: layers["Back Truss"], universe: 1, address: 1, x: 0, y: 4, z: 4.05, rotation: { x: -20, y: -27, z: 0 }, multipatch: fan(false) });
  inputs.push({ number: 82, name: "ACL Out", definition: acl, layerId: layers["Back Truss"], universe: 1, address: 2, x: -3.75, y: 4, z: 4.05, rotation: { x: -20, y: 27, z: 0 }, multipatch: fan(true) });

  let address = 13;
  spread(8, -3.8, 3.8).forEach((x, index) => {
    inputs.push({ number: 101 + index, name: `Profile ${index + 1}`, definition: profile, layerId: layers["Back Truss"], universe: 1, address, x, y: 3.85, z: 4, rotation: { x: -90, y: 0, z: 0 } });
    address += profile.footprint;
  });
  spread(7, -3.25, 3.25).forEach((x, index) => {
    inputs.push({ number: 201 + index, name: `Wash ${index + 1}`, definition: wash, layerId: layers["Back Truss"], universe: 1, address, x, y: 3.85, z: 4, rotation: { x: -90, y: 0, z: 0 } });
    address += wash.footprint;
  });
  address = 1;
  spread(8, -3.5, 3.5).forEach((x, index) => {
    inputs.push({ number: 401 + index, name: `Strip ${index + 1}`, definition: strip, layerId: layers["Back Truss"], universe: 3, address, x, y: 4.15, z: 3.05, rotation: { x: 0, y: 0, z: 0 } });
    address += strip.footprint;
  });
  spread(16, -3.75, 3.75).forEach((x, index) => {
    const group = Math.floor(index / 4);
    const fanIndex = index % 4;
    inputs.push({ number: 301 + index, name: `Floor Spot ${index + 1}`, definition: floor, layerId: layers.Floor, universe: 3, address, x, y: 3.5 - group * 1.4, z: .5, rotation: { x: 70, y: -18 + fanIndex * 12, z: 0 } });
    address += floor.footprint;
  });
  inputs.push({ number: 801, name: "Blinder Left", definition: blinder, layerId: layers["Front Truss"], x: -2, y: -3, z: 4.25, rotation: { x: 0, y: 0, z: 0 } });
  inputs.push({ number: 802, name: "Blinder Right", definition: blinder, layerId: layers["Front Truss"], x: 2, y: -3, z: 4.25, rotation: { x: 0, y: 0, z: 0 } });
  inputs.push({ number: 998, name: "Haze Left", definition: hazer, layerId: layers.Floor, x: -3.5, y: 3.7, z: .2 });
  inputs.push({ number: 999, name: "Haze Right", definition: hazer, layerId: layers.Floor, x: 3.5, y: 3.7, z: .2 });

  const fixtures: Record<number, PatchedFixture> = {};
  for (const input of inputs) {
    const fixture = fixtureBody(input);
    fixtures[input.number] = fixture;
    await put(api, showId, "patched_fixture", fixture.fixture_id, fixture);
  }
  return {
    fixtures,
    profileTargets: targetIds(fixtures, range(101, 108)),
    washTargets: targetIds(fixtures, range(201, 207)),
    floorTargets: targetIds(fixtures, range(301, 316)),
    stripTargets: targetIds(fixtures, range(401, 408)),
  };
}

export async function seedPlannedDemoProgramming(api: ApiDriver, showId: string, rig: PlannedDemoRig): Promise<void> {
  const front = targetIds(rig.fixtures, range(1, 8));
  const profiles = rig.profileTargets;
  const washes = rig.washTargets;
  const floors = rig.floorTargets;
  const strips = rig.stripTargets;
  const groups: Record<string, { name: string; fixtures: string[] }> = {
    "1": { name: "Profiles", fixtures: profiles },
    "2": { name: "Wash", fixtures: washes },
    "3": { name: "LED", fixtures: floors },
    "4": { name: "Strips", fixtures: strips },
    "9": { name: "Front", fixtures: front },
    "11": { name: "Profiles Odd", fixtures: profiles.filter((_, index) => index % 2 === 0) },
    "12": { name: "Profiles Even", fixtures: profiles.filter((_, index) => index % 2 === 1) },
  };
  for (const [id, group] of Object.entries(groups)) await put(api, showId, "group", id, groupBody(group.name, group.fixtures));

  const colorTargets = [...profiles, ...washes, ...floors, ...strips];
  const colors = [
    ["Red", 1, 0, 0], ["Yellow", 1, 1, 0], ["Green", 0, 1, 0], ["Cyan", 0, 1, 1],
    ["Blue", 0, 0, 1], ["Magenta", 1, 0, 1], ["White", 1, 1, 1],
  ] as const;
  for (const [index, [name, red, green, blue]] of colors.entries()) {
    await put(api, showId, "preset", String(index + 1), {
      name, family: "Color", values: Object.fromEntries(colorTargets.map((id) => [id, colorValues(red, green, blue)])), group_values: {},
    });
  }
  const positions = [
    ["Fan Out", .15, .62], ["Mirrored Fan Out", .85, .62], ["Audience", .5, .78], ["Center", .5, .5], ["Crossed", .72, .58],
  ] as const;
  for (const [index, [name, pan, tilt]] of positions.entries()) {
    await put(api, showId, "preset", String(101 + index), {
      name, family: "Position", values: Object.fromEntries([...profiles, ...washes].map((id) => [id, { pan: normalized(pan), tilt: normalized(tilt) }])), group_values: {},
    });
  }
  const profileDefinition = rig.fixtures[101].definition;
  const goboAttribute = profileDefinition.heads.flatMap((head) => head.parameters).find((parameter) => parameter.attribute.startsWith("gobo"))?.attribute;
  if (goboAttribute) {
    for (const [index, value] of [.2, .5, .8].entries()) await put(api, showId, "preset", String(201 + index), {
      name: `Gobo ${index + 1}`, family: "Beam", values: Object.fromEntries(profiles.map((id) => [id, { [goboAttribute]: normalized(value) }])), group_values: {},
    });
  }

  const mainCuelist = crypto.randomUUID();
  const aclCuelist = crypto.randomUUID();
  const washRed = crypto.randomUUID();
  const washBlue = crypto.randomUUID();
  const profileRed = crypto.randomUUID();
  const profileBlue = crypto.randomUUID();
  await put(api, showId, "cue_list", mainCuelist, cueList(mainCuelist, "Demo Main", [
    stateCue(1, "Front and Profiles", [...front, ...profiles].map((fixtureId) => [fixtureId, "intensity", 1])),
    stateCue(2, "Add Wash", [...front, ...profiles, ...washes].map((fixtureId) => [fixtureId, "intensity", 1])),
    stateCue(3, "Red Profiles / Blue Wash", [
      ...[...front, ...profiles, ...washes].map((fixtureId): [string, string, number] => [fixtureId, "intensity", 1]),
      ...profiles.flatMap((fixtureId): Array<[string, string, number]> => [[fixtureId, "color.red", 1], [fixtureId, "color.green", 0], [fixtureId, "color.blue", 0]]),
      ...washes.flatMap((fixtureId): Array<[string, string, number]> => [[fixtureId, "color.red", 0], [fixtureId, "color.green", 0], [fixtureId, "color.blue", 1]]),
    ]),
  ]));
  const aclIn = targetIds(rig.fixtures, [81]);
  const aclOut = targetIds(rig.fixtures, [82]);
  await put(api, showId, "cue_list", aclCuelist, {
    ...cueList(aclCuelist, "ACL Chase", [directCue(1, "ACL In", aclIn, aclOut), directCue(2, "ACL Out", aclOut, aclIn)]),
    mode: "chaser", wrap_mode: "reset", looped: true, speed_group: "A", chaser_step_millis: 500,
  });
  for (const [id, name, groupId, rgb] of [
    [washRed, "Wash Red", "2", [1, 0, 0]], [washBlue, "Wash Blue", "2", [0, 0, 1]],
    [profileRed, "Profile Red", "1", [1, 0, 0]], [profileBlue, "Profile Blue", "1", [0, 0, 1]],
  ] as const) {
    const targets = groupId === "2" ? washes : profiles;
    await put(api, showId, "cue_list", id, cueList(id, name, [colorCue(1, name, targets, rgb)]));
  }

  const playbacks = [
    playback(1, "Profiles Odd", { type: "group", group_id: "11" }),
    playback(2, "Profiles Even", { type: "group", group_id: "12" }),
    playback(3, "Demo Main", { type: "cue_list", cue_list_id: mainCuelist }),
    playback(4, "ACL Chase", { type: "cue_list", cue_list_id: aclCuelist }),
    playback(21, "Wash Red", { type: "cue_list", cue_list_id: washRed }),
    playback(22, "Wash Blue", { type: "cue_list", cue_list_id: washBlue }),
    playback(23, "Profile Red", { type: "cue_list", cue_list_id: profileRed }),
    playback(24, "Profile Blue", { type: "cue_list", cue_list_id: profileBlue }),
  ];
  for (const item of playbacks) await put(api, showId, "playback", String(item.number), item);
  await put(api, showId, "playback_page", "1", { number: 1, name: "Demo", slots: { "1": 1, "2": 2, "3": 3, "4": 4, "21": 21, "22": 22, "23": 23, "24": 24 } });
}

export async function seedPlannedDemoRoutes(api: ApiDriver, showId: string, artnetPort: number, sacnPort: number): Promise<void> {
  const routes = [
    ["demo-artnet-1", { protocol: "art_net", logical_universe: 1, destination_universe: 1, destination: `127.0.0.1:${artnetPort}`, enabled: true, minimum_slots: 128 }],
    ["demo-sacn-2", { protocol: "sacn", logical_universe: 2, destination_universe: 1, destination: `127.0.0.1:${sacnPort}`, enabled: true, minimum_slots: 128 }],
    ["demo-artnet-3", { protocol: "art_net", logical_universe: 3, destination_universe: 1, destination: `127.0.0.1:${artnetPort}`, enabled: true, minimum_slots: 128 }],
  ] as const;
  for (const [id, route] of routes) await put(api, showId, "route", id, route);
}

export async function demoObjects<T = Record<string, any>>(api: ApiDriver, showId: string, kind: string): Promise<Array<VersionedObject<T>>> {
  return api.request<Array<VersionedObject<T>>>("GET", `/api/v1/shows/${showId}/objects/${kind}`, undefined, false);
}

function fixtureBody(input: FixtureInput): PatchedFixture {
  const fixtureId = crypto.randomUUID();
  const mode = modeFor(input.definition);
  const logical_heads = input.definition.heads.filter((head) => !head.shared).map((head) => ({ fixture_id: crypto.randomUUID(), head_index: head.index }));
  const ownerPatch = splitPatches(input.definition, input.universe ?? null, input.address ?? null);
  const primary = ownerPatch.find((patch) => patch.split === 1) ?? ownerPatch[0];
  return {
    fixture_id: fixtureId,
    fixture_number: input.number,
    name: input.name,
    definition: input.definition,
    universe: primary?.universe ?? null,
    address: primary?.address ?? null,
    split_patches: ownerPatch,
    layer_id: input.layerId,
    direct_control: null,
    location: metres(input.x ?? 0, input.y ?? 0, input.z ?? 0),
    rotation: input.rotation ?? { x: 0, y: 0, z: 0 },
    logical_heads,
    multipatch: (input.multipatch ?? []).map((instance) => ({
      id: crypto.randomUUID(), name: instance.name,
      ...(() => { const patches = splitPatches(input.definition, instance.universe ?? null, instance.address ?? null); const first = patches.find((patch) => patch.split === 1) ?? patches[0]; return { universe: first?.universe ?? null, address: first?.address ?? null, split_patches: patches }; })(),
      location: metres(instance.x, instance.y, instance.z), rotation: instance.rotation ?? { x: 0, y: 0, z: 0 },
    })),
    move_in_black_enabled: mode?.splits.some((split) => split.footprint > 0) ?? input.definition.footprint > 0,
    move_in_black_delay_millis: 0,
    highlight_overrides: {},
  };
}

function modeFor(definition: FixtureDefinition) {
  return definition.profile_snapshot?.modes.find((mode) => mode.id === definition.mode_id)
    ?? definition.profile_snapshot?.modes.find((mode) => mode.name === definition.mode);
}

function splitPatches(definition: FixtureDefinition, universe: number | null, address: number | null) {
  const splits = modeFor(definition)?.splits ?? [{ number: 1, footprint: definition.footprint }];
  return splits.map((split, index) => ({ split: split.number, universe: index === 0 ? universe : null, address: index === 0 ? address : null }));
}

function targetIds(fixtures: Record<number, PatchedFixture>, numbers: number[]): string[] {
  return numbers.flatMap((number) => {
    const fixture = fixtures[number];
    if (!fixture) throw new Error(`Demo fixture ${number} is missing`);
    return fixture.logical_heads.length ? fixture.logical_heads.map((head) => head.fixture_id) : [fixture.fixture_id];
  });
}

function groupBody(name: string, fixtures: string[]) {
  return { name, fixtures, color: null, icon: null, derived_from: null, frozen_from: null, programming: {}, master: 1, playback_fader: null };
}

function colorValues(red: number, green: number, blue: number) {
  return { "color.red": normalized(red), "color.green": normalized(green), "color.blue": normalized(blue) };
}

function normalized(value: number) { return { kind: "normalized", value }; }

function stateCue(number: number, name: string, changes: Array<[string, string, number]>) {
  return { id: crypto.randomUUID(), number, name, cue_only: false, changes: changes.map(([fixture_id, attribute, value]) => ({ fixture_id, attribute, value: normalized(value), automatic_restore: false })), group_changes: [], fade_millis: 1_000, delay_millis: 0, trigger: { type: "manual" }, phasers: [] };
}

function directCue(number: number, name: string, on: string[], off: string[]) {
  return { id: crypto.randomUUID(), number, name, cue_only: false, changes: [...on.map((fixture_id) => ({ fixture_id, attribute: "intensity", value: normalized(1), automatic_restore: false })), ...off.map((fixture_id) => ({ fixture_id, attribute: "intensity", value: normalized(0), automatic_restore: false }))], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] };
}

function colorCue(number: number, name: string, targets: string[], rgb: readonly [number, number, number]) {
  return {
    id: crypto.randomUUID(), number, name, cue_only: false,
    changes: targets.flatMap((fixture_id) => [
      ["color.red", rgb[0]], ["color.green", rgb[1]], ["color.blue", rgb[2]],
    ].map(([attribute, value]) => ({ fixture_id, attribute, value: normalized(value as number), automatic_restore: false }))),
    group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [],
  };
}

function cueList(id: string, name: string, cues: unknown[]) {
  return { id, name, cues, mode: "sequence", priority: 0, looped: false, intensity_priority_mode: "htp", wrap_mode: "off", restart_mode: "first_cue", force_cue_timing: false, disable_cue_timing: false, chaser_step_millis: 1_000, chaser_xfade_millis: 0, speed_group: null, speed_multiplier: 1 };
}

function playback(number: number, name: string, target: Record<string, unknown>) {
  const buttons = target.type === "group"
    ? ["select", "flash", "select_dereferenced"]
    : ["go", "go_minus", "flash"];
  return { number, name, target, buttons, button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false };
}

function metres(x: number, y: number, z: number) { return { x: Math.round(x * 1_000), y: Math.round(y * 1_000), z: Math.round(z * 1_000) }; }
function range(first: number, last: number) { return Array.from({ length: last - first + 1 }, (_, index) => first + index); }
function spread(count: number, first: number, last: number) { return Array.from({ length: count }, (_, index) => count === 1 ? first : first + (last - first) * index / (count - 1)); }
async function put(api: ApiDriver, showId: string, kind: string, id: string, body: unknown) { await api.request("PUT", `/api/v1/shows/${showId}/objects/${kind}/${id}`, body, true, 0); }
