import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchContractContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	object,
	objects,
	putObject,
} from "../support/catalog";

export async function emptyPlaybackPage(api: ApiDriver) {
	const page = (await objects<any>(api, "playback_page")).find(
		(item) => item.body.number === 1,
	);
	await putObject(
		api,
		"playback_page",
		"1",
		{ ...(page?.body ?? { number: 1, name: "Main" }), slots: {} },
		page?.revision ?? 0,
	);
}

export async function installCompactGroups(api: ApiDriver) {
	const fixtures = await fixtureIdsByNumber(api);
	const existing = await objects<any>(api, "group");
	for (const [id, numbers] of [
		["1", [1, 2, 3, 4]],
		["2", [5, 6, 7, 8]],
		["3", [9, 10, 11, 12]],
	] as const) {
		const current = existing.find((item) => item.id === id);
		await putObject(
			api,
			"group",
			id,
			{
				...(current?.body ?? {}),
				id,
				name: `Group ${id}`,
				fixtures: numbers.map((number) => fixtures[number]),
				derived_from: null,
				frozen_from: null,
				programming: current?.body.programming ?? {},
				master: 1,
				playback_fader: current?.body.playback_fader ?? null,
			},
			current?.revision ?? 0,
		);
	}
}

export async function expectNewRecordedCuelist(
	api: ApiDriver,
	before: Set<string>,
	cueCount: number,
) {
	await expect
		.poll(
			async () =>
				(await objects<any>(api, "cue_list")).filter(
					(item) => !before.has(item.id),
				).length,
		)
		.toBe(1);
	const recorded = (await objects<any>(api, "cue_list")).find(
		(item) => !before.has(item.id),
	);
	expect(recorded).toBeDefined();
	await expect
		.poll(
			async () =>
				(await object<any>(api, "cue_list", recorded!.id)).body.cues.length,
		)
		.toBe(cueCount);
	return object<any>(api, "cue_list", recorded!.id);
}

export async function playbackAtSlot(
	api: ApiDriver,
	slot: number,
): Promise<number> {
	let playbackNumber: number | undefined;
	await expect
		.poll(async () => {
			playbackNumber = (await object<any>(api, "playback_page", "1")).body
				.slots[String(slot)];
			return playbackNumber;
		})
		.toEqual(expect.any(Number));
	return playbackNumber!;
}

export async function playbackState(api: ApiDriver): Promise<any> {
	return api.request("GET", "/api/v1/playbacks");
}

export async function runtime(
	api: ApiDriver,
	playbackNumber: number,
): Promise<any> {
	return (await playbackState(api)).active.find(
		(item: any) => item.playback_number === playbackNumber,
	);
}

export function logicalSlots(frame: any, count: number): number[] {
	return (
		frame.universes.find((universe: any) => universe.universe === 1)?.slots ??
		[]
	).slice(0, count);
}

export function slot(frame: any, fixtureNumber: number): number {
	return (
		frame.universes.find((universe: any) => universe.universe === 1)?.slots[
			fixtureNumber - 1
		] ?? 0
	);
}

export async function assertCompactGroupSequence(
	bench: BenchContractContext["bench"],
	expected: number[][],
	advance: () => Promise<unknown>,
) {
	for (const groups of expected) {
		await advance();
		expect(logicalSlots(await bench.tick(0), 12)).toEqual(
			groups.flatMap((value) => Array(4).fill(value * 255)),
		);
	}
}

export type ValueOptions = {
	automatic_restore?: boolean;
	fade_millis?: number;
	delay_millis?: number;
};
export type CueOptions = {
	fade_millis?: number;
	delay_millis?: number;
	trigger?: any;
};
export type FixtureValue = readonly [string, string, number, ValueOptions?];
export type GroupValue = readonly [string, string, number];

export function fixtureCue(
	number: number,
	values: readonly FixtureValue[],
	options: CueOptions = {},
) {
	return {
		id: crypto.randomUUID(),
		number,
		name: `Cue ${number}`,
		changes: values.map(([fixture_id, attribute, value, timing]) => ({
			fixture_id,
			attribute,
			value: { kind: "normalized", value },
			automatic_restore: timing?.automatic_restore ?? false,
			...(timing?.fade_millis == null
				? {}
				: { fade_millis: timing.fade_millis }),
			...(timing?.delay_millis == null
				? {}
				: { delay_millis: timing.delay_millis }),
		})),
		group_changes: [],
		fade_millis: options.fade_millis ?? 0,
		delay_millis: options.delay_millis ?? 0,
		trigger: options.trigger ?? { type: "manual" },
		phasers: [],
	};
}

export function groupCue(
	number: number,
	values: readonly GroupValue[],
	options: CueOptions = {},
) {
	return {
		id: crypto.randomUUID(),
		number,
		name: `Cue ${number}`,
		changes: [],
		group_changes: values.map(([group_id, attribute, value]) => ({
			group_id,
			attribute,
			value: { kind: "normalized", value },
		})),
		fade_millis: options.fade_millis ?? 0,
		delay_millis: options.delay_millis ?? 0,
		trigger: options.trigger ?? { type: "manual" },
		phasers: [],
	};
}

export async function installPlaybackSequence(
	api: ApiDriver,
	playbackNumber: number,
	cues: any[],
	options: {
		name?: string;
		priority?: number;
		intensity_priority_mode?: "htp" | "ltp";
		auto_off?: boolean;
	} = {},
) {
	const id = crypto.randomUUID();
	await putObject(api, "cue_list", id, {
		id,
		name: options.name ?? `Cuelist ${playbackNumber}`,
		priority: options.priority ?? 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		intensity_priority_mode: options.intensity_priority_mode ?? "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues,
	});
	const existingPlayback = (await objects<any>(api, "playback")).find(
		(item) => item.id === String(playbackNumber),
	);
	await putObject(
		api,
		"playback",
		String(playbackNumber),
		{
			number: playbackNumber,
			name: options.name ?? `Playback ${playbackNumber}`,
			target: { type: "cue_list", cue_list_id: id },
			buttons: ["go_minus", "go", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: options.auto_off ?? false,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
			presentation_icon: null,
			presentation_image: null,
		},
		existingPlayback?.revision ?? 0,
	);
	const page = (await objects<any>(api, "playback_page")).find(
		(item) => item.body.number === 1,
	);
	await putObject(
		api,
		"playback_page",
		page?.id ?? "1",
		{
			...(page?.body ?? { number: 1, name: "Main" }),
			slots: { ...(page?.body.slots ?? {}), [playbackNumber]: playbackNumber },
		},
		page?.revision ?? 0,
	);
	return { id, playbackNumber };
}
