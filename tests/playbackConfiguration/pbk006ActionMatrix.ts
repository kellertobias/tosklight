import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchUiContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	controls,
	definition,
	installPlaybacks,
	playbackAt,
	poolAction,
	prepareShow,
	pressButton,
	programmer,
	setSpeedRates,
	updatePlayback,
} from "./helpers";

async function prepareSpecializedPlaybacks({
	api,
	bench,
}: Pick<BenchUiContext, "api" | "bench">): Promise<void> {
	await prepareShow(api, bench, "pbk-006-masters", "default-stage");
	await setSpeedRates(api, [120, 96, 72, 60, 48]);
	await installPlaybacks(
		api,
		[
			definition(
				61,
				"Speed A",
				{ type: "speed_group", group: "A" },
				{
					buttons: ["double", "half", "learn"],
					fader: "learned_percentage",
				},
			),
			definition(
				62,
				"Group 1",
				{ type: "group", group_id: "1" },
				{ buttons: ["select", "select_dereferenced", "flash"] },
			),
			definition(
				63,
				"Grand",
				{ type: "grand_master" },
				{ buttons: ["blackout", "pause_dynamics", "flash"] },
			),
			definition(
				64,
				"Programmer Fade",
				{ type: "programmer_fade" },
				{ buttons: ["double", "half", "off"] },
			),
			definition(
				65,
				"Cue Fade",
				{ type: "cue_fade" },
				{ buttons: ["double", "half", "off"] },
			),
		],
		{ 1: 61, 2: 62, 3: 63, 4: 64, 5: 65 },
	);
}

async function verifySpeedButtons(
	api: ApiDriver,
	bench: BenchUiContext["bench"],
): Promise<void> {
	expect((await playbackAt(api, 1, 1)).body).toMatchObject({
		buttons: ["double", "half", "learn"],
		fader: "learned_percentage",
	});
	await pressButton(api, 61, 1);
	expect((await controls(api)).speed_groups[0].manual_bpm).toBe(240);
	await pressButton(api, 61, 2);
	expect((await controls(api)).speed_groups[0].manual_bpm).toBe(120);
	await updatePlayback(api, 1, (current) => ({
		...current,
		buttons: ["pause", "half", "learn"],
	}));
	await pressButton(api, 61, 1);
	expect((await controls(api)).speed_groups[0].paused).toBe(true);
	await pressButton(api, 61, 1);
	expect((await controls(api)).speed_groups[0].paused).toBe(false);
	await pressButton(api, 61, 3);
	await bench.tick(500);
	await pressButton(api, 61, 3);
	expect((await controls(api)).speed_groups[0].manual_bpm).toBeCloseTo(120, 3);
}

async function verifySpeedFaders(api: ApiDriver): Promise<void> {
	await updatePlayback(api, 1, (current) => ({
		...current,
		fader: "direct_bpm",
	}));
	for (const [position, bpm] of [
		[0, 0],
		[0.5, 150],
		[1, 300],
	] as const) {
		await poolAction(api, 61, "master", { value: position });
		const speed = (await controls(api)).speed_groups[0];
		expect(speed.effective_bpm).toBeCloseTo(bpm, 3);
		expect(speed.paused).toBe(position === 0);
	}
	await setSpeedRates(api, [120, 96, 72, 60, 48]);
	await updatePlayback(api, 1, (current) => ({
		...current,
		fader: "learned_percentage",
	}));
	for (const [position, bpm] of [
		[0, 0],
		[0.5, 60],
		[1, 120],
	] as const) {
		await poolAction(api, 61, "master", { value: position });
		expect((await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(
			bpm,
			3,
		);
	}
	await updatePlayback(api, 1, (current) => ({
		...current,
		fader: "centered_relative",
	}));
	for (const [position, bpm] of [
		[0, 30],
		[0.5, 120],
		[1, 480],
	] as const) {
		await poolAction(api, 61, "master", { value: position });
		expect((await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(
			bpm,
			3,
		);
	}
	const neighbors = (await controls(api)).speed_groups
		.slice(1)
		.map((speed: any) => speed.manual_bpm);
	expect(neighbors).toEqual([96, 72, 60, 48]);
}

async function verifyGroupControl(api: ApiDriver): Promise<void> {
	await poolAction(api, 62, "master", { value: 0.4 });
	let groupControl = (await controls(api)).groups.find(
		(group: any) => group.id === "1",
	);
	expect(groupControl.flash_level).toBe(0);
	expect(groupControl.master).toBeCloseTo(0.4, 5);
	await pressButton(api, 62, 1);
	expect((await programmer(api)).selection_expression).toMatchObject({
		type: "live_group",
		group_id: "1",
	});
	await pressButton(api, 62, 2);
	expect((await programmer(api)).selection_expression).toEqual({
		type: "static",
	});
	await pressButton(api, 62, 3, true);
	groupControl = (await controls(api)).groups.find(
		(group: any) => group.id === "1",
	);
	expect(groupControl.flash_level).toBe(1);
	expect(groupControl.master).toBeCloseTo(0.4, 5);
	await pressButton(api, 62, 3, false);
	groupControl = (await controls(api)).groups.find(
		(group: any) => group.id === "1",
	);
	expect(groupControl.flash_level).toBe(0);
	expect(groupControl.master).toBeCloseTo(0.4, 5);
}

async function verifyGrandMaster(api: ApiDriver): Promise<void> {
	await poolAction(api, 63, "master", { value: 0.3 });
	await pressButton(api, 63, 1);
	let grandMaster = (await controls(api)).grand_master;
	expect(grandMaster.blackout).toBe(true);
	expect(grandMaster.level).toBeCloseTo(0.3, 5);
	expect(grandMaster.effective_level).toBeCloseTo(0.3, 5);
	await pressButton(api, 63, 3, true);
	grandMaster = (await controls(api)).grand_master;
	expect(grandMaster.flash_active).toBe(true);
	expect(grandMaster.level).toBeCloseTo(0.3, 5);
	expect(grandMaster.effective_level).toBe(1);
	await pressButton(api, 63, 3, false);
	grandMaster = (await controls(api)).grand_master;
	expect(grandMaster.flash_active).toBe(false);
	expect(grandMaster.level).toBeCloseTo(0.3, 5);
	expect(grandMaster.effective_level).toBeCloseTo(0.3, 5);
	await pressButton(api, 63, 2);
	expect((await controls(api)).grand_master.dynamics_paused).toBe(true);
	await pressButton(api, 63, 2);
	expect((await controls(api)).grand_master.dynamics_paused).toBe(false);
}

async function verifyFadeMasters(api: ApiDriver): Promise<void> {
	await poolAction(api, 64, "master", { value: 0.25 });
	await poolAction(api, 65, "master", { value: 0.25 });
	expect(await controls(api)).toMatchObject({
		programmer_fade_millis: 5_000,
		cue_fade_millis: 15_000,
	});
	await pressButton(api, 64, 1);
	await pressButton(api, 65, 1);
	expect(await controls(api)).toMatchObject({
		programmer_fade_millis: 10_000,
		cue_fade_millis: 30_000,
	});
	await pressButton(api, 64, 2);
	await pressButton(api, 65, 2);
	expect(await controls(api)).toMatchObject({
		programmer_fade_millis: 5_000,
		cue_fade_millis: 15_000,
	});
	await pressButton(api, 64, 3);
	await pressButton(api, 65, 3);
	expect(await controls(api)).toMatchObject({
		programmer_fade_millis: 0,
		cue_fade_millis: 0,
	});
}

export async function runPbk006ActionMatrixScenario({
	api,
	bench,
}: BenchUiContext): Promise<void> {
	await prepareSpecializedPlaybacks({ api, bench });
	await verifySpeedButtons(api, bench);
	await verifySpeedFaders(api);
	await verifyGroupControl(api);
	await verifyGrandMaster(api);
	await verifyFadeMasters(api);
}
