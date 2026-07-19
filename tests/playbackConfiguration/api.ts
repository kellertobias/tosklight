import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "../support/catalog";
import type {
	PlaybackDefinition,
	PlaybackTarget,
	PreparedShow,
} from "./models";

export function definition(
	number: number,
	name: string,
	target: PlaybackTarget,
	overrides: Partial<PlaybackDefinition> = {},
): PlaybackDefinition {
	const defaults =
		target.type === "speed_group"
			? {
					buttons: ["double", "half", "learn"] as [string, string, string],
					fader: "learned_percentage",
				}
			: target.type === "group"
				? {
						buttons: ["select", "select_dereferenced", "flash"] as [
							string,
							string,
							string,
						],
						fader: "master",
					}
				: target.type === "grand_master"
					? {
							buttons: ["blackout", "pause_dynamics", "flash"] as [
								string,
								string,
								string,
							],
							fader: "master",
						}
					: target.type === "programmer_fade" || target.type === "cue_fade"
						? {
								buttons: ["double", "half", "off"] as [string, string, string],
								fader: "master",
							}
						: {
								buttons: ["go_minus", "go", "flash"] as [
									string,
									string,
									string,
								],
								fader: "master",
							};
	return {
		number,
		name,
		target,
		buttons: defaults.buttons,
		button_count: 3,
		fader: defaults.fader,
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
		...overrides,
	};
}

export async function prepareShow(
	api: ApiDriver,
	bench: any,
	name: string,
	fixture: "compact-rig" | "default-stage",
	levels = [0.2, 0.8, 0.4],
	fadeMillis = 0,
	delayMillis = 0,
): Promise<PreparedShow> {
	const show = await loadCanonicalCopy(api, bench, name, fixture);
	const fixtures = await fixtureIdsByNumber(api);
	const existingGroups = await objects<any>(api, "group");
	for (const [id, groupName, members] of [
		["1", "All Fixtures", Object.values(fixtures)],
		["3", "Front Fixtures", Object.values(fixtures).slice(0, 4)],
	] as const) {
		if (existingGroups.some((group) => group.id === id)) continue;
		await putObject(api, "group", id, {
			id,
			name: groupName,
			fixtures: members,
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: Number(id),
		});
	}
	const cueListId = await createCueList(
		api,
		fixtures,
		"Configured Sequence",
		levels,
		fadeMillis,
		delayMillis,
		[1, 2],
	);
	return { showId: show.id, cueListId, fixtures };
}

export async function createCueList(
	api: ApiDriver,
	fixtures: Record<number, string>,
	name: string,
	levels: number[],
	fadeMillis: number,
	delayMillis: number,
	fixtureNumbers: number[],
	includeGroupChange = true,
): Promise<string> {
	const id = crypto.randomUUID();
	await putObject(api, "cue_list", id, {
		id,
		name,
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues: levels.map((level, index) => ({
			id: crypto.randomUUID(),
			number: index + 1,
			name: `Cue ${index + 1}`,
			changes: fixtureNumbers.map((number) => ({
				fixture_id: fixtures[number],
				attribute: "intensity",
				value: { kind: "normalized", value: level },
				automatic_restore: false,
			})),
			group_changes:
				includeGroupChange && index === 0
					? [
							{
								group_id: "3",
								attribute: "intensity",
								value: { kind: "normalized", value: level },
								fade_millis: fadeMillis,
								delay_millis: delayMillis,
							},
						]
					: [],
			fade_millis: fadeMillis,
			delay_millis: delayMillis,
			trigger: { type: "manual" },
			phasers: [],
		})),
	});
	return id;
}

export async function installPlaybacks(
	api: ApiDriver,
	definitions: PlaybackDefinition[],
	slots: Record<number, number>,
): Promise<void> {
	for (const playback of definitions) {
		const current = (await objects(api, "playback")).find(
			(item) => item.id === String(playback.number),
		);
		await putObject(
			api,
			"playback",
			String(playback.number),
			playback,
			current?.revision ?? 0,
		);
	}
	await writePage(
		api,
		1,
		Object.fromEntries(
			Object.entries(slots).map(([slot, number]) => [String(slot), number]),
		),
	);
}

export async function writePage(
	api: ApiDriver,
	number: number,
	slots: Record<string, number>,
): Promise<void> {
	const current = (await objects<any>(api, "playback_page")).find(
		(item) => item.id === String(number),
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

export async function playbackAt(api: ApiDriver, page: number, slot: number) {
	const pageState = await pageObject(api, page);
	const number = pageState.body.slots[String(slot)];
	expect(number).toBeDefined();
	return object<PlaybackDefinition>(api, "playback", String(number));
}

export async function saveSlot(
	api: ApiDriver,
	page: number,
	slot: number,
	playback: PlaybackDefinition,
) {
	const pageState = await pageObject(api, page);
	const currentNumber = pageState.body.slots[String(slot)];
	const currentPlayback =
		currentNumber == null
			? undefined
			: (await objects<PlaybackDefinition>(api, "playback")).find(
					(item) => item.id === String(currentNumber),
				);
	return api.request<any>(
		"PUT",
		`/api/v1/playback-pages/${page}/slots/${slot}`,
		{
			playback,
			expected_playback_revision: currentPlayback?.revision ?? 0,
			expected_page_revision: pageState.revision,
		},
	);
}

export async function clearSlot(api: ApiDriver, page: number, slot: number) {
	const pageState = await pageObject(api, page);
	const playback = await playbackAt(api, page, slot);
	return api.request<any>(
		"DELETE",
		`/api/v1/playback-pages/${page}/slots/${slot}`,
		{
			expected_playback_revision: playback.revision,
			expected_page_revision: pageState.revision,
		},
	);
}

export async function updatePlayback(
	api: ApiDriver,
	slot: number,
	mutate: (current: PlaybackDefinition) => PlaybackDefinition,
) {
	const current = await playbackAt(api, 1, slot);
	return saveSlot(api, 1, slot, mutate(current.body));
}

export async function setFirstButton(
	api: ApiDriver,
	slot: number,
	action: string,
): Promise<void> {
	await updatePlayback(api, slot, (current) => ({
		...current,
		buttons: [action, current.buttons[1], current.buttons[2]],
	}));
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

export async function pressButton(
	api: ApiDriver,
	number: number,
	button = 1,
	pressed = true,
) {
	return poolAction(api, number, "button", {
		button,
		pressed,
		surface: "physical",
	});
}

export async function playbackSnapshot(api: ApiDriver) {
	return api.request<any>("GET", "/api/v1/playbacks");
}

export async function activePlayback(api: ApiDriver, number: number) {
	const active = (await playbackSnapshot(api)).active.find(
		(item: any) => item.playback_number === number,
	);
	expect(active).toBeDefined();
	return active;
}

export async function controls(api: ApiDriver) {
	return (await playbackSnapshot(api)).authoritative_controls;
}

export async function logicalDmx(api: ApiDriver): Promise<number[]> {
	const snapshot = await api.request<any>(
		"GET",
		"/api/v1/dmx",
		undefined,
		false,
	);
	return logicalUniverse(snapshot);
}

export function logicalUniverse(snapshot: {
	universes: Array<{ universe: number; slots: number[] }>;
}): number[] {
	return (
		snapshot.universes.find((universe) => universe.universe === 1)?.slots ?? []
	);
}

export async function audit(api: ApiDriver): Promise<any[]> {
	return api.request<any[]>("GET", "/api/v1/audit?after=0");
}

export async function inertSnapshot(api: ApiDriver, number: number) {
	const playback = await object<PlaybackDefinition>(
		api,
		"playback",
		String(number),
	);
	const state = await playbackSnapshot(api);
	return {
		object: playback,
		pool: state.pool,
		pages: state.pages,
		active: state.active,
		selected_playback: state.selected_playback,
		audit: await audit(api),
		dmx: await logicalDmx(api),
	};
}

export async function setSpeedRates(
	api: ApiDriver,
	rates: number[],
): Promise<void> {
	const response = await api.request<any>("GET", "/api/v1/configuration");
	await api.request("PUT", "/api/v1/configuration", {
		...response.configuration,
		speed_groups_bpm: rates,
		speed_group_sound_to_light:
			response.configuration.speed_group_sound_to_light.map((sound: any) => ({
				...sound,
				enabled: false,
			})),
	});
}
