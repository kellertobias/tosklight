import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { LightBench } from "../apps/control-ui/e2e/bench/lightBench";
import { fixtureIdsByNumber, objects, putObject } from "./support/catalog";

export async function assignMatterRestartPlayback(api: ApiDriver): Promise<{
	page: number;
	slot: number;
	playbackNumber: number;
}> {
	const pages = await objects<any>(api, "playback_page");
	const pagesByNumber = new Map<number, (typeof pages)[number]>(
		pages.map((page) => [Number(page.body.number), page]),
	);
	const pageNumber = Array.from({ length: 127 }, (_, index) => index + 1).find(
		(candidate) => {
			const assigned = new Set(
				Object.keys(pagesByNumber.get(candidate)?.body.slots ?? {}).map(Number),
			);
			return Array.from({ length: 127 }, (_, index) => index + 1).some(
				(slot) => !assigned.has(slot),
			);
		},
	);
	expect(pageNumber).toBeDefined();
	const page = pagesByNumber.get(pageNumber!);
	const assigned = new Set(Object.keys(page?.body.slots ?? {}).map(Number));
	const slot = Array.from({ length: 127 }, (_, index) => index + 1).find(
		(candidate) => !assigned.has(candidate),
	);
	expect(slot).toBeDefined();
	const existingCueList = (await objects<any>(api, "cue_list"))[0];
	const cueListId =
		existingCueList?.id ?? (await createMatterRestartCueList(api));
	const result = await api.request<any>(
		"PUT",
		`/api/v1/playback-pages/${pageNumber}/slots/${slot}`,
		{
			playback: {
				number: 0,
				name: "Matter restart persistence",
				target: { type: "cue_list", cue_list_id: cueListId },
				buttons: ["toggle", "none", "none"],
				button_count: 1,
				fader: "master",
				has_fader: false,
				go_activates: true,
				auto_off: false,
				xfade_millis: 0,
				color: "#20c997",
				flash_release: "release_all",
				protect_from_swap: false,
			},
			expected_playback_revision: 0,
			expected_page_revision: page?.revision ?? 0,
		},
	);
	return {
		page: pageNumber!,
		slot: slot!,
		playbackNumber: result.playback.number,
	};
}

export async function createMatterRestartCueList(
	api: ApiDriver,
): Promise<string> {
	const fixture = (await objects<any>(api, "patched_fixture"))[0];
	expect(fixture).toBeDefined();
	const id = crypto.randomUUID();
	await putObject(api, "cue_list", id, {
		id,
		name: "Matter restart persistence",
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues: [
			{
				id: crypto.randomUUID(),
				number: 1,
				name: "Matter on",
				changes: [
					{
						fixture_id: fixture.body.fixture_id,
						attribute: "intensity",
						value: { kind: "normalized", value: 1 },
						automatic_restore: false,
					},
				],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				phasers: [],
			},
		],
	});
	return id;
}
export async function installTimeCuelists(
	api: ApiDriver,
	chaserFixture: string,
	phaserFixture: string,
): Promise<string> {
	const chaserId = crypto.randomUUID();
	await putObject(api, "cue_list", chaserId, {
		id: chaserId,
		name: "Virtual Chaser",
		priority: 0,
		mode: "chaser",
		looped: true,
		chaser_step_millis: 1_000,
		speed_group: "A",
		intensity_priority_mode: "htp",
		wrap_mode: "tracking",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues: [0.25, 0.5, 0.75, 1].map((level, index) =>
			cue(index + 1, chaserFixture, level),
		),
	});
	const phaserId = crypto.randomUUID();
	const phaserCue = cue(1, phaserFixture, 0);
	phaserCue.phasers = [
		{
			fixture_ids: [phaserFixture],
			group_ids: [],
			attribute: "intensity",
			phaser: {
				mode: "absolute",
				steps: [
					{ position: 0, value: 0, curve_to_next: "linear" },
					{ position: 0.5, value: 1, curve_to_next: "linear" },
				],
				cycles_per_minute: 60,
				phase_start_degrees: 0,
				phase_end_degrees: 0,
				width: 1,
			},
		},
	];
	await putObject(api, "cue_list", phaserId, {
		id: phaserId,
		name: "Virtual Phaser",
		priority: 1,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: true,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues: [phaserCue],
	});
	await putObject(
		api,
		"playback",
		"1",
		playback(1, chaserId, "Virtual Chaser"),
	);
	await putObject(
		api,
		"playback",
		"2",
		playback(2, phaserId, "Virtual Phaser"),
	);
	await putObject(api, "playback", "3", {
		number: 3,
		name: "Dynamics Control",
		target: { type: "grand_master" },
		buttons: ["blackout", "flash", "pause_dynamics"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	});
	return chaserId;
}

export async function restartPlaybackRun(
	api: ApiDriver,
	bench: LightBench,
	showId: string,
	numbers: number[],
): Promise<void> {
	for (const number of [1, 2])
		await api
			.request("POST", `/api/v1/cuelists/${number}/off`, {})
			.catch(() => undefined);
	await api.request("POST", `/api/v1/shows/${showId}/open`, {
		transition: "hold_current",
	});
	await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
	for (const number of numbers)
		await api.request("POST", `/api/v1/cuelists/${number}/go`, {});
}

export async function playbackRuntime(
	api: ApiDriver,
	number: number,
): Promise<any> {
	const state = await api.request<any>("GET", "/api/v1/playbacks");
	const runtime = state.active.find(
		(item: any) => item.playback_number === number,
	);
	expect(runtime).toBeDefined();
	return runtime;
}

export async function installGroupCue(
	api: ApiDriver,
	groupId: string,
	level: number,
): Promise<string> {
	const id = crypto.randomUUID();
	const first = cue(1, (await fixtureIdsByNumber(api))[1], 0);
	first.changes = [];
	first.group_changes = [
		{
			group_id: groupId,
			attribute: "intensity",
			value: { kind: "normalized", value: level },
		},
	];
	await putObject(
		api,
		"cue_list",
		id,
		sequence(id, "SHOW-001 Cuelist", [first]),
	);
	await putObject(api, "playback", "1", playback(1, id, "SHOW-001 Playback"));
	return id;
}

export async function installSequence(
	api: ApiDriver,
	fixtureId: string,
): Promise<string> {
	const id = crypto.randomUUID();
	await putObject(
		api,
		"cue_list",
		id,
		sequence(id, "Legacy migration", [
			cue(1, fixtureId, 0.25),
			cue(2, fixtureId, 0.75),
		]),
	);
	return id;
}

export function sequence(id: string, name: string, cues: any[]): any {
	return {
		id,
		name,
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues,
	};
}

export function cue(number: number, fixtureId: string, level: number): any {
	return {
		id: crypto.randomUUID(),
		number,
		name: `Cue ${number}`,
		changes: [
			{
				fixture_id: fixtureId,
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
	};
}

export function playback(number: number, cueListId: string, name: string): any {
	return {
		number,
		name,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go_minus", "go", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: false,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}
