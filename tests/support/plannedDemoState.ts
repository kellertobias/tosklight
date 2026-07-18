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

const PLANNED_DEMO_PACKAGES = [
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

export async function ensurePlannedDemoFixtureLibrary(api: ApiDriver): Promise<void> {
  let profiles = await api.request<FixtureProfile[]>("GET", "/api/v1/fixture-profiles", undefined, false);
  for (const [manufacturer, name, archive] of PLANNED_DEMO_PACKAGES) {
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
}

interface FixtureInput {
  phase: PlannedDemoPatchPhase;
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

export type PlannedDemoPatchPhase = "stage" | "acl" | "strips" | "front" | "floor" | "house" | "remaining";
const ALL_PATCH_PHASES: readonly PlannedDemoPatchPhase[] = ["stage", "acl", "strips", "front", "floor", "house", "remaining"];

export async function seedPlannedDemoPatch(
  api: ApiDriver,
  showId: string,
  layers: Record<string, string>,
  phases: readonly PlannedDemoPatchPhase[] = ALL_PATCH_PHASES,
  selectedLayerIds?: readonly string[],
): Promise<PlannedDemoRig> {
  await ensurePlannedDemoFixtureLibrary(api);
  const legacyLibrary = await api.request<FixtureDefinition[]>("GET", "/api/v1/fixture-library", undefined, false);
  const profiles = await api.request<FixtureProfile[]>("GET", "/api/v1/fixture-profiles", undefined, false);
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
  inputs.push({
    phase: "stage", number: venueNumber++, name: "Stage", definition: stage, layerId: layers.Stage, x: -3, y: -1.5, z: 0,
    multipatch: [-3, -1, 1, 3].flatMap((x) => [-1.5, -.5, .5, 1.5].map((y) => ({ x, y })))
      .filter(({ x, y }) => x !== -3 || y !== -1.5)
      .map(({ x, y }, index) => ({ name: `Stage ${index + 2}`, x, y, z: 0 })),
  });
  for (const [name, y] of [["Back Truss", 4], ["Mid Truss", 0], ["Front Truss", -3]] as const) {
    inputs.push({
      phase: "stage", number: venueNumber++, name, definition: truss, layerId: layers.Stage, x: -3, y, z: 4.15,
      multipatch: [-1, 1, 3].map((x, index) => ({ name: `${name} ${index + 2}`, x, y, z: 4.15 })),
    });
  }
  [-1.5, -.5, .5, 1.5].forEach((x, index) => inputs.push({ phase: "strips", number: venueNumber++, name: `Pipe ${index + 1}`, definition: pipe, layerId: layers.Stage, x, y: 4.2, z: 2.9, rotation: { x: 0, y: 90, z: 0 } }));
  [-2, 2].forEach((x, index) => inputs.push({ phase: "stage", number: venueNumber++, name: `Curtain ${index + 1}`, definition: curtain, layerId: layers.Stage, x, y: 4.3, z: 2.5 }));

  const frontTargets = spread(4, -3.8, 3.8);
  [-3.8, -3.533, -3.267, -3].forEach((x, index) => inputs.push({ phase: "front", number: index + 1, name: `Front Left ${index + 1}`, definition: fresnel, layerId: layers["Front Truss"], universe: 2, address: index + 1, x, y: -3, z: 4, rotation: aimFixtureAt({ x, y: -3, z: 4 }, { x: frontTargets[index], y: 1.5, z: 0 }) }));
  [3, 3.267, 3.533, 3.8].forEach((x, index) => inputs.push({ phase: "front", number: index + 5, name: `Front Right ${index + 1}`, definition: fresnel, layerId: layers["Front Truss"], universe: 2, address: index + 7, x, y: -3, z: 4, rotation: aimFixtureAt({ x, y: -3, z: 4 }, { x: frontTargets[index], y: 1.5, z: 0 }) }));
  inputs.push({
    phase: "house", number: 99, name: "House Light", definition: dimmer, layerId: layers["House Lights"], universe: 2, address: 13, x: 0, y: -6, z: 4,
    multipatch: [14, 15, 16].map((address) => ({ name: `House Light ${address - 12}`, universe: 2, address, x: 0, y: -5 + address - 14, z: 4 })),
  });
  inputs.push({
    phase: "house", number: 98, name: "House Mood", definition: dimmer, layerId: layers["House Lights"], universe: 2, address: 17,
    multipatch: [18, 19, 20, 21, 22, 23, 24].map((address) => ({ name: `House Mood ${address - 16}`, universe: 2, address, x: -3 + address - 18, y: -4, z: 3 })),
  });
  const aclInPositions = spread(8, -.4, .4);
  const aclInTargets = spread(8, -3.8, 3.8);
  const aclInInstances = aclInPositions.map((x, index) => ({
    name: `ACL In ${index + 1}`, x, y: 4, z: 4.3,
    rotation: aimFixtureAt({ x, y: 4, z: 4.3 }, { x: aclInTargets[index], y: -2, z: 0 }),
  }));
  const aclOutPositions = [...spread(4, -3.8, -3), ...spread(4, 3, 3.8)];
  const aclOutTargets = [...spread(4, -4, 2), ...spread(4, -2, 4)];
  const aclOutInstances = aclOutPositions.map((x, index) => ({
    name: `ACL Out ${index + 1}`, x, y: 4, z: 4.3,
    rotation: aimFixtureAt({ x, y: 4, z: 4.3 }, { x: aclOutTargets[index], y: -2, z: 0 }),
  }));
  inputs.push({ phase: "acl", number: 81, name: "ACL In", definition: acl, layerId: layers["Back Truss"], universe: 1, address: 1, x: aclInInstances[0].x, y: aclInInstances[0].y, z: aclInInstances[0].z, rotation: aclInInstances[0].rotation, multipatch: aclInInstances.slice(1) });
  inputs.push({ phase: "acl", number: 82, name: "ACL Out", definition: acl, layerId: layers["Back Truss"], universe: 1, address: 2, x: aclOutInstances[0].x, y: aclOutInstances[0].y, z: aclOutInstances[0].z, rotation: aclOutInstances[0].rotation, multipatch: aclOutInstances.slice(1) });

  let address = 13;
  spread(8, -3.8, 3.8).forEach((x, index) => {
    inputs.push({ phase: "remaining", number: 101 + index, name: `Profile ${index + 1}`, definition: profile, layerId: layers["Back Truss"], universe: 1, address, x, y: 3.85, z: 4, rotation: { x: -90, y: 0, z: 0 } });
    address += profile.footprint;
  });
  spread(7, -3.25, 3.25).forEach((x, index) => {
    inputs.push({ phase: "remaining", number: 201 + index, name: `Wash ${index + 1}`, definition: wash, layerId: layers["Back Truss"], universe: 1, address, x, y: 3.85, z: 4, rotation: { x: -90, y: 0, z: 0 } });
    address += wash.footprint;
  });
  address = 1;
  const stripPositions = [-1.5, -.5, .5, 1.5].flatMap((x) => [{ x, z: 3.45 }, { x, z: 2.2 }]);
  stripPositions.forEach(({ x, z }, index) => {
    inputs.push({ phase: "strips", number: 401 + index, name: `Strip ${index + 1}`, definition: strip, layerId: layers["Back Truss"], universe: 3, address, x, y: 4.05, z, rotation: { x: 0, y: 90, z: 0 } });
    address += strip.footprint;
  });
  const floorGroupCenters = [-3, -1, 1, 3];
  floorGroupCenters.flatMap((center) => spread(4, center - .3, center + .3).map((x, fanIndex) => ({ center, x, fanIndex }))).forEach(({ center, x, fanIndex }, index) => {
    const targetX = center + spread(4, -1.1, 1.1)[fanIndex];
    inputs.push({ phase: "floor", number: 301 + index, name: `Floor Spot ${index + 1}`, definition: floor, layerId: layers.Floor, universe: 3, address, x, y: 1.6, z: .2, rotation: aimFixtureAt({ x, y: 1.6, z: .2 }, { x: targetX, y: -3, z: 4 }) });
    address += floor.footprint;
  });
  inputs.push({ phase: "remaining", number: 801, name: "Blinder Left", definition: blinder, layerId: layers["Front Truss"], x: -2, y: -3, z: 4.25, rotation: { x: 0, y: 0, z: 0 } });
  inputs.push({ phase: "remaining", number: 802, name: "Blinder Right", definition: blinder, layerId: layers["Front Truss"], x: 2, y: -3, z: 4.25, rotation: { x: 0, y: 0, z: 0 } });
  inputs.push({ phase: "remaining", number: 998, name: "Haze Left", definition: hazer, layerId: layers.Floor, x: -3.5, y: 3.7, z: .2 });
  inputs.push({ phase: "remaining", number: 999, name: "Haze Right", definition: hazer, layerId: layers.Floor, x: 3.5, y: 3.7, z: .2 });

  const existingByNumber = new Map(
    (await demoObjects<PatchedFixture>(api, showId, "patched_fixture"))
      .flatMap((item) => item.body.fixture_number != null
        ? [[item.body.fixture_number, item.body] as const]
        : item.body.virtual_fixture_number != null
          ? [[10_000 + item.body.virtual_fixture_number, item.body] as const]
          : []),
  );
  const fixtures: Record<number, PatchedFixture> = {};
  const selectedPhases = new Set(phases);
  const selectedLayers = selectedLayerIds ? new Set(selectedLayerIds) : null;
  for (const input of inputs) {
    const fixture = fixtureBody(input, existingByNumber.get(input.number));
    fixtures[input.number] = fixture;
    if (selectedPhases.has(input.phase) && (!selectedLayers || selectedLayers.has(input.layerId))) {
      await put(api, showId, "patched_fixture", fixture.fixture_id, fixture);
    }
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
  // Red and Blue are deliberately absent here: the narrated workflow records Color 1 and
  // Color 5 through the desk. Seeding them as well made the first visible store an
  // unexplained overwrite and left a duplicate command-line Color preset.
  const colors = [
    [2, "Yellow", 1, 1, 0], [3, "Green", 0, 1, 0], [4, "Cyan", 0, 1, 1],
    [6, "Magenta", 1, 0, 1], [7, "White", 1, 1, 1],
  ] as const;
  for (const [number, name, red, green, blue] of colors) {
    await put(api, showId, "preset", `2.${number}`, {
      name, family: "Color", values: Object.fromEntries(colorTargets.map((id) => [id, colorValues(red, green, blue)])), group_values: {},
    });
  }
  const positions = [
    ["Fan Out", .15, .62], ["Mirrored Fan Out", .85, .62], ["Audience", .5, .78], ["Center", .5, .5], ["Crossed", .72, .58],
  ] as const;
  for (const [index, [name, pan, tilt]] of positions.entries()) {
    await put(api, showId, "preset", `3.${index + 1}`, {
      name, family: "Position", values: Object.fromEntries([...profiles, ...washes].map((id) => [id, { pan: normalized(pan), tilt: normalized(tilt) }])), group_values: {},
    });
  }
  const profileDefinition = rig.fixtures[101].definition;
  const goboAttribute = profileDefinition.heads.flatMap((head) => head.parameters).find((parameter) => parameter.attribute.startsWith("gobo"))?.attribute;
  if (goboAttribute) {
    for (const [index, value] of [.2, .5, .8].entries()) await put(api, showId, "preset", `4.${index + 1}`, {
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

function fixtureBody(input: FixtureInput, existing?: PatchedFixture): PatchedFixture {
  const fixtureId = existing?.fixture_id ?? crypto.randomUUID();
  const mode = modeFor(input.definition);
  const visualOnly = (mode?.splits.every((split) => split.footprint === 0) ?? input.definition.footprint === 0);
  const headIndexes = input.definition.heads.filter((head) => !head.shared).map((head) => head.index);
  const logical_heads = headIndexes.map((headIndex) => existing?.logical_heads.find((head) => head.head_index === headIndex) ?? ({ fixture_id: crypto.randomUUID(), head_index: headIndex }));
  const ownerPatch = splitPatches(input.definition, input.universe ?? null, input.address ?? null);
  const primary = ownerPatch.find((patch) => patch.split === 1) ?? ownerPatch[0];
  return {
    fixture_id: fixtureId,
    fixture_number: visualOnly ? null : input.number,
    virtual_fixture_number: visualOnly ? input.number - 10_000 : null,
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
    multipatch: (input.multipatch ?? []).map((instance, index) => ({
      id: existing?.multipatch?.[index]?.id ?? crypto.randomUUID(), name: instance.name,
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
export function aimFixtureAt(origin: { x: number; y: number; z: number }, target: { x: number; y: number; z: number }) {
  const dx = target.x - origin.x;
  const dy = target.y - origin.y;
  const dz = target.z - origin.z;
  const distance = Math.hypot(dx, dy, dz) || 1;
  return {
    x: Math.atan2(dy, -dz) * 180 / Math.PI,
    y: Math.asin(Math.max(-1, Math.min(1, dx / distance))) * 180 / Math.PI,
    z: 0,
  };
}
function range(first: number, last: number) { return Array.from({ length: last - first + 1 }, (_, index) => first + index); }
function spread(count: number, first: number, last: number) { return Array.from({ length: count }, (_, index) => count === 1 ? first : first + (last - first) * index / (count - 1)); }
async function put(api: ApiDriver, showId: string, kind: string, id: string, body: unknown) {
  let revision = 0;
  for (let attempt = 0; attempt < 4; attempt++) {
    try {
      await api.request("PUT", `/api/v1/shows/${showId}/objects/${kind}/${id}`, body, true, revision);
      return;
    } catch (error) {
      const current = error instanceof Error
        ? /revision conflict: expected \d+, current (\d+)/.exec(error.message)?.[1]
        : undefined;
      if (!current) throw error;
      revision = Number(current);
    }
  }
  throw new Error(`Could not write demo ${kind} ${id} after repeated revision conflicts`);
}
