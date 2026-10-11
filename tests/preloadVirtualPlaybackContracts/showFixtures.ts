import type { PlaybackAddress } from "../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { ApiDriver } from "../bench/core/api";
import { type BenchContractContext, expect } from "../bench/core/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "../support/catalog";
import type { PlaybackPreloadScope } from "../../apps/light-desktop/src/api/playbackConfigurationModels";
import { activeShowId } from "../support/catalog";
import type { Configuration, PlaybackSpec, Prepared } from "./types";

export async function prepare(
	api: ApiDriver,
	bench: BenchContractContext["bench"],
	name: string,
	specs: PlaybackSpec[],
	sourcePositionsByPosition: Record<number, number>,
): Promise<Prepared> {
	const show = await loadCanonicalCopy(api, bench, name);
	const fixtures = await fixtureIdsByNumber(api);
	return installOnCurrentShow(api, show.id, fixtures, specs, sourcePositionsByPosition);
}

export async function installOnCurrentShow(
	api: ApiDriver,
	showId: string,
	fixtures: Record<number, string>,
	specs: PlaybackSpec[],
	sourcePositionsByPosition: Record<number, number>,
): Promise<Prepared> {
	const cueLists: Record<number, string> = {};
	for (const spec of specs) {
		const cueListId = crypto.randomUUID();
		cueLists[spec.position] = cueListId;
		await putObject(api, "cue_list", cueListId, {
			id: cueListId,
			name: spec.name ?? `Preload ${spec.position}`,
			priority: 0,
			mode: "sequence",
			looped: false,
			chaser_step_millis: 1_000,
			speed_group: null,
			cues: (spec.levels ?? [1]).map((level, index) => ({
				id: crypto.randomUUID(),
				number: index + 1,
				name: `Cue ${index + 1}`,
				changes: [
					{
						fixture_id: fixtures[spec.fixture],
						attribute: "intensity",
						value: { kind: "normalized", value: level },
						automatic_restore: false,
					},
				],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
			})),
		});
	}
	const definitions = Object.fromEntries(specs.map(spec => [spec.position, playbackDefinition(spec, cueLists[spec.position]) ]));
	const currentPage = (await objects<any>(api,"playback_page")).find(entry=>entry.id==="1");
	const assignments = { ...(currentPage?.body.assignments ?? {}), ...definitions, ...Object.fromEntries(Object.entries(sourcePositionsByPosition).map(([position, source]) => [position, definitions[source]])) };
	await putObject(api, "playback_page", "1", { number: 1, name: currentPage?.body.name ?? "Main", assignments },currentPage?.revision ?? 0);
	return { showId, fixtures, cueLists };
}

export function playbackDefinition(spec: PlaybackSpec, cueListId: string) {
	return {
		name: spec.name ?? `Preload ${spec.position}`,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: spec.buttons ?? ["go", "go_minus", "flash"],
		button_count: spec.buttonCount ?? 3,
		fader: "master",
		has_fader: spec.hasFader ?? true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

/** Copy real sparse assignments between exact page positions. */
export async function copyPageAssignments(api: ApiDriver, page: number, sources: Record<number, number>) {
 const sourcePage = await pageObject(api, 1);
 const current = (await objects<any>(api, "playback_page")).find(entry => entry.id === String(page));
 const assignments = { ...(current?.body.assignments ?? {}) };
 for (const [position, sourcePosition] of Object.entries(sources)) {
  const definition = sourcePage.body.assignments[sourcePosition];
  if (!definition) throw new Error(`No source Playback 1.${sourcePosition}`);
  assignments[position] = { ...definition, buttons: [definition.buttons[0], "none", "none"], button_count: 1, has_fader: false };
 }
 await putObject(api, "playback_page", String(page), {number:page,name:current?.body.name ?? `Page ${page}`,assignments},current?.revision ?? 0);
}

export async function pageObject(api: ApiDriver, page: number) {
	return object<any>(api, "playback_page", String(page));
}

export async function configuration(api: ApiDriver): Promise<Configuration> {
	return (await api.request<any>("GET", "/api/v2/configuration")).configuration;
}

export async function setCaptureScope(api: ApiDriver, programmerCapture: boolean, playbackScope: PlaybackPreloadScope, programmerFade = 3000, cueFade = 3000) {
 await api.request("POST", "/api/v2/configuration/update", { request_id:crypto.randomUUID(), patch:{programmer_fade_millis:programmerFade, command_line_at_uses_programmer_fade:programmerCapture, sequence_master_fade_millis:cueFade,preload_programmer_changes:programmerCapture} });
 const showId = await activeShowId(api);
 const snapshot = await api.request<any>("GET", "/api/v2/playback-preload-scope", undefined, true, undefined, {showId});
 await api.request("POST", "/api/v2/playback-preload-scope/update", {request_id:crypto.randomUUID(),expected_revision:snapshot.revision,scope:playbackScope},true,undefined,{showId});
}

export async function assignedPlaybackAction(
	api: ApiDriver,
	address: { page: number; position: number },
	action: string,
	body: Record<string, unknown> = {},
) {
	return api.playbackAction(address, action, body);
}

export async function playbacks(api: ApiDriver): Promise<any> {
	return api.request("GET", "/api/v2/playback-overview");
}

export async function activeAssignedPlayback(api: ApiDriver, address: PlaybackAddress): Promise<any | undefined> {
 const showId=await activeShowId(api);
 const snapshot = await api.request<any>("POST", "/api/v2/playback-runtime/snapshot", {identities:[{kind:"playback",...address}]},true,undefined,{showId,deskId:api.session!.desk.id});
 const projection = snapshot.projections[0];
 const runtime = projection?.target === "cue_list" ? projection.runtime : null;
 return runtime?.current?.number == null ? undefined : {...runtime,identity:projection.identity,current_cue_number:runtime.current.number,loaded_cue_number:runtime.loaded?.number ?? null};
}

export async function paneAction(api: ApiDriver, address: PlaybackAddress, action: "go" | "toggle" | "on" | "off", body: Record<string,unknown> = {}) {
 return api.playbackAction(address,action,body);
}

export async function firstGroupFixture(
	api: ApiDriver,
	id: string,
): Promise<string> {
	const group = await object<any>(api, "group", id);
	expect(group.body.fixtures.length).toBeGreaterThan(0);
	return group.body.fixtures[0];
}

export async function distinctGroupFixtures(
	api: ApiDriver,
	broadId: string,
	subsetId: string,
): Promise<[string, string]> {
	const broad = (await object<any>(api, "group", broadId)).body
		.fixtures as string[];
	const subset = (await object<any>(api, "group", subsetId)).body
		.fixtures as string[];
	const broadOnly = broad.find((fixture) => !subset.includes(fixture));
	expect(broadOnly).toBeDefined();
	expect(subset[0]).toBeDefined();
	return [broadOnly!, subset[0]];
}

export async function visualizationLevel(
	api: ApiDriver,
	fixtureId: string,
	attribute = "intensity",
): Promise<number> {
	const snapshot = await api.request<any>(
		"GET",
		"/api/v2/output/visualization",
	);
	const value = snapshot.values.find(
		(entry: any) =>
			entry.fixture_id === fixtureId && entry.attribute === attribute,
	)?.value;
	return typeof value === "number" ? value : (value?.value ?? 0);
}

export async function audit(api: ApiDriver, after = 0): Promise<any[]> {
	return api.request("GET", `/api/v2/audit?after=${after}`);
}

export function summarizePlaybackState(snapshot: any, numbers: number[]) {
	return snapshot.active
		.filter((entry: any) => entry.playback_address?.page === 1 && numbers.includes(entry.playback_address.position))
		.map((entry: any) => ({
			address: entry.playback_address,
			cue: entry.current_cue_number,
			enabled: entry.enabled,
			temporary: entry.temporary_active,
		}))
		.sort((left: any, right: any) => left.address.position - right.address.position);
}
