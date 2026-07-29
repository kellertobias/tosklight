import { expect, test } from "./bench/core/fixtures";
import { activeShowId, loadCanonicalCopy } from "./support/catalog";
import { installPlannedDemoGroups } from "./support/plannedDemoGroups";
import { installPlannedDemoDynamics } from "./support/plannedDemoDynamics";
import { installPlannedDemoPatch } from "./support/plannedDemoPatch";
import { installPlannedDemoPlaybacks } from "./support/plannedDemoPlaybacks";
import { installPlannedDemoPresets } from "./support/plannedDemoPresets";

test("DEMO-GENERATOR-001 @api › installs the exact Plan 76 lighting patch from one manifest", async ({
  api,
  bench,
}) => {
  await loadCanonicalCopy(api, bench, "plan-76-generator", "default-stage");
  const showId = await activeShowId(api);
  const layerNames = [
    "Back Truss", "Mid Truss", "Front Truss", "Floor",
    "Audience", "Auxiliary", "House Lights",
  ];
  const layers = Object.fromEntries(layerNames.map((name, index) => [
    name,
    `00000000-0000-4000-8100-${(index + 1).toString(16).padStart(12, "0")}`,
  ]));
  for (const [index, [name, id]] of Object.entries(layers).entries())
    await api.seedShowObject(showId, "patch_layer", id, { id, name, order: index }, 0);

  const generated = await installPlannedDemoPatch(api, showId, layers);
  expect(generated).toMatchObject({
    fixtureRecords: 262,
    physicalInstances: 301,
    firstUniverse: 1,
  });
  expect(generated.lastUniverse).toBeGreaterThan(1);
  expect(generated.occupiedSlots).toBe(3_783);

  const acls = generated.fixtures
    .filter((fixture) => fixture.fixture_number >= 601 && fixture.fixture_number <= 604);
  expect(acls.map((fixture) => fixture.name)).toEqual([
    "Back Centre ACL", "Back Split ACL", "Mid Split ACL", "Front Split ACL",
  ]);
  expect(acls.every((fixture) => fixture.multipatch.length === 7)).toBe(true);
  const frontInstances = [acls[3], ...acls[3].multipatch];
  expect(frontInstances.filter((fixture) => fixture.location.x < 0)).toHaveLength(4);
  expect(frontInstances.filter((fixture) => fixture.location.x > 0)).toHaveLength(4);

  const groupSpecs = await installPlannedDemoGroups(api, showId, generated.fixtures);
  expect(groupSpecs).toHaveLength(38);
  const groups = await api.showObjects<any>(showId, "group");
  expect(groups).toHaveLength(38);
  expect(groups.find((group) => group.body.name === "Show")?.body.fixtures).toHaveLength(208);
  expect(groups.find((group) => group.body.name === "Aux Show")?.body.fixtures).toHaveLength(24);

  expect(await installPlannedDemoPresets(api, showId, generated.fixtures)).toEqual({
    colors: 13,
    positions: 7,
    beam: 10,
  });
  const presets = await api.showObjects<any>(showId, "preset");
  expect(presets).toHaveLength(30);
  expect(presets.filter((preset) => preset.body.family === "Color")).toHaveLength(13);
  expect(presets.find((preset) => preset.body.name === "Tungsten White")).toBeDefined();

  const topology = await installPlannedDemoPlaybacks(api, showId, generated.fixtures);
  expect(topology.cuelists).toHaveLength(7);
  expect(topology.playbacks).toHaveLength(13);
  const cuelists = await api.showObjects<any>(showId, "cue_list");
  expect(cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body)
    .toMatchObject({ mode: "chaser", speed_group: "D", looped: true });
  expect(cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body.cues).toHaveLength(4);

  const dynamics = await installPlannedDemoDynamics(api, showId);
  expect(dynamics).toHaveLength(30);
  const storedDynamics = await api.showObjects<any>(showId, "dynamic");
  expect(storedDynamics).toHaveLength(30);
  expect(storedDynamics.find((dynamic) => dynamic.body.name === "Wash Row Waterfall")
    ?.body.phase.ordering).toEqual({ type: "grid_linear", angle_degrees: 90 });
  const [page] = await api.showObjects<any>(showId, "playback_page");
  expect(Object.keys(page.body.virtual_playbacks)).toHaveLength(30);
});
