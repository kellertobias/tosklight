import type { ApiDriver } from "../bench/core/api";
import { installPlannedDemoDynamics } from "./plannedDemoDynamics";
import { installPlannedDemoGroups } from "./plannedDemoGroups";
import { installPlannedDemoLayout } from "./plannedDemoLayouts";
import { putPlannedDemoObject } from "./plannedDemoObjects";
import { installPlannedDemoPatch } from "./plannedDemoPatch";
import { installPlannedDemoPlaybacks } from "./plannedDemoPlaybacks";
import { installPlannedDemoPresets } from "./plannedDemoPresets";

export async function generatePlannedDemo(
  api: ApiDriver,
  showId: string,
  layers: Readonly<Record<string, string>> = {},
) {
  const resolvedLayers = await ensurePlannedDemoLayers(api, showId, layers);
  const patch = await installPlannedDemoPatch(api, showId, resolvedLayers);
  const groups = await installPlannedDemoGroups(api, showId, patch.fixtures);
  const presets = await installPlannedDemoPresets(api, showId, patch.fixtures);
  const topology = await installPlannedDemoPlaybacks(api, showId, patch.fixtures);
  const dynamics = await installPlannedDemoDynamics(api, showId);
  const layout = await installPlannedDemoLayout(api, showId);
  return { patch, groups, presets, topology, dynamics, layout };
}

const LAYER_NAMES = [
  "Back Truss", "Mid Truss", "Front Truss", "Floor",
  "Audience", "Auxiliary", "House Lights",
] as const;

async function ensurePlannedDemoLayers(
  api: ApiDriver,
  showId: string,
  provided: Readonly<Record<string, string>>,
) {
  const existing = await api.showObjects<any>(showId, "patch_layer");
  const resolved: Record<string, string> = { ...provided };
  for (const [index, name] of LAYER_NAMES.entries()) {
    const current = existing.find((layer) => layer.body.name === name);
    if (current) {
      resolved[name] = current.id;
      continue;
    }
    const id = `00000000-0000-4000-8400-${(index + 1).toString(16).padStart(12, "0")}`;
    await putPlannedDemoObject(api, showId, "patch_layer", id, { id, name, order: index });
    resolved[name] = id;
  }
  return resolved;
}
