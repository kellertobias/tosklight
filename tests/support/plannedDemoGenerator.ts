import type { ApiDriver } from "../bench/core/api";
import { installPlannedDemoDynamics } from "./plannedDemoDynamics";
import { installPlannedDemoGroups } from "./plannedDemoGroups";
import { installPlannedDemoLayout } from "./plannedDemoLayouts";
import { installPlannedDemoPatch } from "./plannedDemoPatch";
import { installPlannedDemoPlaybacks } from "./plannedDemoPlaybacks";
import { installPlannedDemoPresets } from "./plannedDemoPresets";

export async function generatePlannedDemo(
  api: ApiDriver,
  showId: string,
  layers: Readonly<Record<string, string>>,
) {
  const patch = await installPlannedDemoPatch(api, showId, layers);
  const groups = await installPlannedDemoGroups(api, showId, patch.fixtures);
  const presets = await installPlannedDemoPresets(api, showId, patch.fixtures);
  const topology = await installPlannedDemoPlaybacks(api, showId, patch.fixtures);
  const dynamics = await installPlannedDemoDynamics(api, showId);
  const layout = await installPlannedDemoLayout(api, showId);
  return { patch, groups, presets, topology, dynamics, layout };
}
