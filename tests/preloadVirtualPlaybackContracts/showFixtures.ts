import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchContractContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "../support/catalog";
import type { Configuration, PlaybackSpec, Prepared } from "./types";

export async function prepare(
	api: ApiDriver,
	bench: BenchContractContext["bench"],
	name: string,
	specs: PlaybackSpec[],
	slots: Record<number, number>,
): Promise<Prepared> {
	await loadCanonicalCopy(api, bench, name);
	const fixtures = await fixtureIdsByNumber(api);
	return installOnCurrentShow(api, fixtures, specs, slots);
}

export async function installOnCurrentShow(
	api: ApiDriver,
	fixtures: Record<number, string>,
	specs: PlaybackSpec[],
	slots: Record<number, number>,
): Promise<Prepared> {
	const cueLists: Record<number, string> = {};
	for (const spec of specs) {
		const cueListId = crypto.randomUUID();
		cueLists[spec.number] = cueListId;
		await putObject(api, "cue_list", cueListId, {
			id: cueListId,
			name: spec.name ?? `Preload ${spec.number}`,
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
				phasers: [],
			})),
		});
		await putObject(
			api,
			"playback",
			String(spec.number),
			playbackDefinition(spec, cueListId),
		);
	}
	await writePage(
		api,
		1,
		Object.fromEntries(
			Object.entries(slots).map(([slot, number]) => [String(slot), number]),
		),
	);
	return { fixtures, cueLists };
}

export function playbackDefinition(spec: PlaybackSpec, cueListId: string) {
	return {
		number: spec.number,
		name: spec.name ?? `Preload ${spec.number}`,
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

export async function writePage(
	api: ApiDriver,
	number: number,
	slots: Record<string, number>,
) {
	const current = (await objects<any>(api, "playback_page")).find(
		(entry) => entry.id === String(number),
	);
	await putObject(
		api,
		"playback_page",
		String(number),
		{ number, name: number === 1 ? "Main" : `Page ${number}`, slots },
		current?.revision ?? 0,
	);
}

export async function pageObject(api: ApiDriver, page: number) {
	return object<any>(api, "playback_page", String(page));
}

export async function configuration(api: ApiDriver): Promise<Configuration> {
	return (await api.request<any>("GET", "/api/v1/configuration")).configuration;
}

export async function setCaptureMask(
	api: ApiDriver,
	programmerCapture: boolean,
	physicalCapture: boolean,
	virtualCapture: boolean,
	programmerFade = 3_000,
	cueFade = 3_000,
) {
	const current = await configuration(api);
	await api.request("PUT", "/api/v1/configuration", {
		...current,
		programmer_fade_millis: programmerFade,
		sequence_master_fade_millis: cueFade,
		preload_programmer_changes: programmerCapture,
		preload_physical_playback_actions: physicalCapture,
		preload_virtual_playback_actions: virtualCapture,
	});
}

export async function poolAction<T = any>(
	api: ApiDriver,
	number: number,
	action: string,
	body: Record<string, unknown> = {},
): Promise<T> {
	return api.request<T>(
		action === "master" ? "PUT" : "POST",
		`/api/v1/playback-pool/${number}/${action}`,
		body,
	);
}

export async function playbacks(api: ApiDriver): Promise<any> {
	return api.request("GET", "/api/v1/playbacks");
}

export async function activePlayback(
	api: ApiDriver,
	number: number,
): Promise<any | undefined> {
	return (await playbacks(api)).active.find(
		(entry: any) => entry.playback_number === number,
	);
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
	const snapshot = await api.request<any>("GET", "/api/v1/visualization");
	const value = snapshot.values.find(
		(entry: any) =>
			entry.fixture_id === fixtureId && entry.attribute === attribute,
	)?.value;
	return typeof value === "number" ? value : (value?.value ?? 0);
}

export async function audit(api: ApiDriver, after = 0): Promise<any[]> {
	return api.request("GET", `/api/v1/audit?after=${after}`);
}

export function summarizePlaybackState(snapshot: any, numbers: number[]) {
	return snapshot.active
		.filter((entry: any) => numbers.includes(entry.playback_number))
		.map((entry: any) => ({
			number: entry.playback_number,
			cue: entry.current_cue_number,
			enabled: entry.enabled,
			temporary: entry.temporary_active,
		}))
		.sort((left: any, right: any) => left.number - right.number);
}
