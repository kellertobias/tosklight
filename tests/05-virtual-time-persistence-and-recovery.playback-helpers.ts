import type { ApiDriver } from "./bench/core/api";
import { expect } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { fixtureIdsByNumber, objects, putObject } from "./support/catalog";
import { configurePlaybackSlot } from "./support/playbackTopology";

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
	const result = await configurePlaybackSlot(api, pageNumber!, slot!, {
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
	});
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
			},
		],
	});
	return id;
}
export async function installTimeCuelists(
	api: ApiDriver,
	chaserFixture: string,
	dynamicFixture: string,
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
	const dynamicId = crypto.randomUUID();
	const value = (level: number) => ({ type: "value", value: level });
	const pwm = {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear",
		decay_interpolation: "linear",
	};
	const dynamic = {
		id: dynamicId,
		pool_number: 1,
		revision: 1,
		name: "Virtual Dynamic",
		color: null,
		icon: null,
		target_binding: { type: "frozen_targets", targets: [dynamicFixture] },
		lanes: [
			{
				id: crypto.randomUUID(),
				attribute: "intensity",
				mode: "keyframes",
				keyframes: {
					points: [
						{ position: 0, source: value(0), interpolation: "linear" },
						{ position: 0.5, source: value(1), interpolation: "linear" },
					],
					size: 1,
				},
				max_min: {
					minimum: value(0),
					maximum: value(1),
					function: "sinus",
					size: 1,
					pwm,
				},
				middle_amplitude: {
					middle: { type: "current" },
					amplitude: 0.5,
					function: "sinus",
					size: 1,
					pwm,
				},
				speed_multiplier: { numerator: 1, denominator: 1 },
				width: 1,
				random_group_id: null,
			},
		],
		random_groups: [],
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 1_000 },
		default_activation: "start_now",
	};
	await putObject(api, "dynamic", dynamicId, dynamic);
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
		{
			number: 2,
			name: "Virtual Dynamic",
			target: {
				type: "dynamic",
				assignment: {
					dynamic: {
						dynamic_id: dynamicId,
						last_known_pool_number: 1,
						embedded_fallback: { definition: dynamic },
					},
					revision: 1,
					target_scope: null,
					fader_mode: "size_and_master",
					priority: 1,
					activation_override: null,
					resume_policy: "follow_dynamic",
					local_speed_multiplier: { numerator: 1, denominator: 1 },
					learned_duration_millis: null,
					crossfade_non_intensity: false,
					auto_off_at_zero: false,
					auto_off_flash_release: false,
					auto_off_full_control: true,
				},
			},
			buttons: ["off", "pause", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
		},
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
		await api.playbackNumberAction(number, "off", {}).catch(() => undefined);
	await api.openShow(showId, {
		transition: "hold_current",
	});
	await api.request("POST", "/api/v2/test/clock/reset", undefined, false);
	for (const number of numbers)
		await api.playbackNumberAction(number, number === 2 ? "on" : "go", {});
}

export async function playbackRuntime(
	api: ApiDriver,
	number: number,
): Promise<any> {
	const state = await api.request<any>("GET", "/api/v2/playback-overview");
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
