import type { ApiDriver } from "../bench/core/api";
import { installPlannedDemoDynamics } from "./plannedDemoDynamics";
import { installPlannedDemoGroups } from "./plannedDemoGroups";
import { installPlannedDemoLayout } from "./plannedDemoLayouts";
import { installPlannedDemoMedia } from "./plannedDemoMedia";
import { putPlannedDemoObject } from "./plannedDemoObjects";
import { installPlannedDemoPatch } from "./plannedDemoPatch";
import { installPlannedDemoPlaybacks } from "./plannedDemoPlaybacks";
import { installPlannedDemoPresets } from "./plannedDemoPresets";
import { installPlannedDemoScenery } from "./plannedDemoScenery";
import { installPlannedDemoVirtualPlaybackExclusionZones } from "./plannedDemoVirtualPlaybackZones";

export async function generatePlannedDemo(
	api: ApiDriver,
	showId: string,
	layers: Readonly<Record<string, string>> = {},
) {
	const resolvedLayers = await ensurePlannedDemoLayers(api, showId, layers);
	const outputRoutes = await installPlannedDemoOutputRoutes(api, showId);
	const scenery = await installPlannedDemoScenery(api, showId, resolvedLayers);
	const patch = await installPlannedDemoPatch(api, showId, resolvedLayers);
	const media = await installPlannedDemoMedia(api, showId);
	const groups = await installPlannedDemoGroups(api, showId, patch.fixtures);
	const presets = await installPlannedDemoPresets(api, showId, patch.fixtures);
	const topology = await installPlannedDemoPlaybacks(
		api,
		showId,
		patch.fixtures,
	);
	const dynamics = await installPlannedDemoDynamics(api, showId);
	const virtualPlaybackExclusionZones =
		await installPlannedDemoVirtualPlaybackExclusionZones(api, showId);
	const layout = await installPlannedDemoLayout(api, showId);
	return {
		outputRoutes,
		patch,
		media,
		scenery,
		groups,
		presets,
		topology,
		dynamics,
		virtualPlaybackExclusionZones,
		layout,
	};
}

export async function installPlannedDemoOutputRoutes(
	api: ApiDriver,
	showId: string,
) {
	for (const route of await api.showObjects<any>(showId, "route"))
		await api.deleteSeededShowObject(showId, "route", route.id, route.revision);
	const routes = Array.from({ length: 9 }, (_, index) => {
		const universe = index + 1;
		return {
			id: `planned-demo-artnet-${universe}`,
			body: {
				protocol: "art_net",
				logical_universe: universe,
				destination_universe: universe,
				delivery_mode: "unicast",
				destination: "127.0.0.1:6454",
				enabled: true,
				minimum_slots: 128,
			},
		};
	});
	for (const route of routes)
		await putPlannedDemoObject(api, showId, "route", route.id, route.body);
	return routes;
}

const LAYER_NAMES = [
	"Stage & Venue",
	"Trusses",
	"Profile Stage",
	"Profile Audience",
	"Profile Auxilliary",
	"Wash Stage",
	"Wash Audience",
	"Wash Auxilliary",
	"LED PAR Stage",
	"LED PAR Audience",
	"LED PAR Auxilliary",
	"Audience Beams",
	"Sunstrips",
	"Conventional Light",
	"Media Servers",
	"Lasers",
	"Sparklers",
	"Flame Jets",
] as const;

export async function ensurePlannedDemoLayers(
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
		await putPlannedDemoObject(api, showId, "patch_layer", id, {
			id,
			name,
			order: index,
		});
		resolved[name] = id;
	}
	return resolved;
}
