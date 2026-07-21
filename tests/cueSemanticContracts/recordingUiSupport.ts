import type { BenchUiContext } from "../../apps/control-ui/e2e/bench/fixtures";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { clearProgrammerValues } from "../../apps/control-ui/e2e/bench/programmerValues";
import { object, objects, putObject } from "../support/catalog";
import {
	emptyPlaybackPage,
	expectNewRecordedCuelist,
	logicalSlots,
	playbackAtSlot,
	rounded,
	runtime,
} from "./support";

export const cue001Ui = async (
	{ api, bench, desk, page }: BenchUiContext,
	state: { completed: boolean; showId: string },
) => {
	await emptyPlaybackPage(api);
	const beforeCuelists = new Set(
		(await objects(api, "cue_list")).map((item) => item.id),
	);

	await api.executeCommandLine("GROUP 1 AT 100");
	await desk.open(bench.baseUrl);
	await page.locator(".mode-toggle").click();
	await page.getByRole("button", { name: "REC", exact: true }).click();
	await page
		.getByRole("button", { name: "Playback representation page 1 playback 1" })
		.click();

	const stored = await expectNewRecordedCuelist(api, beforeCuelists, 1);
	const playbackNumber = await playbackAtSlot(api, 1);
	await expect
		.poll(async () => runtime(api, playbackNumber))
		.toMatchObject({
			current_cue_number: 1,
			enabled: true,
			flash: false,
		});
	expect(stored.body.cues[0].group_changes).toMatchObject([
		{
			group_id: "1",
			attribute: "intensity",
			value: { kind: "normalized", value: 1 },
		},
	]);
	expect(logicalSlots(await bench.tick(3_000), 8)).toEqual([
		...Array(4).fill(255),
		...Array(4).fill(0),
	]);

	for (const [button, group, level] of [
		["GO +", "2", 1],
		["GO −", "3", 1],
		["FLASH", "1", 0.5],
	] as const) {
		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.executeCommandLine(`GROUP ${group} AT ${level * 100}`);
		await page.getByRole("button", { name: "REC", exact: true }).click();
		const card = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: stored.body.name });
		await card.getByRole("button", { name: button, exact: true }).click();
		const expectedCueCount = button === "GO +" ? 2 : button === "GO −" ? 3 : 4;
		await expect
			.poll(
				async () =>
					(await object<any>(api, "cue_list", stored.id)).body.cues.length,
			)
			.toBe(expectedCueCount);
		await expect
			.poll(async () => runtime(api, playbackNumber))
			.toMatchObject({
				current_cue_number: expectedCueCount,
				enabled: true,
				flash: false,
			});
	}

	const definition = await object<any>(api, "playback", String(playbackNumber));
	await putObject(
		api,
		"playback",
		String(playbackNumber),
		{ ...definition.body, buttons: ["toggle", "on", "off"] },
		definition.revision,
	);
	for (const [index, [button, group, level]] of [
		["TOGGLE", "2", 0.2],
		["ON", "3", 0.3],
		["OFF", "1", 0.4],
	].entries()) {
		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.executeCommandLine(`GROUP ${group} AT ${level * 100}`);
		await page.getByRole("button", { name: "REC", exact: true }).click();
		const card = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: stored.body.name });
		await card.getByRole("button", { name: button, exact: true }).click();
		const expectedCueCount = index + 5;
		await expect
			.poll(
				async () =>
					(await object<any>(api, "cue_list", stored.id)).body.cues.length,
			)
			.toBe(expectedCueCount);
		await expect
			.poll(async () => runtime(api, playbackNumber))
			.toMatchObject({
				current_cue_number: expectedCueCount,
				enabled: true,
				flash: false,
			});
	}

	const final = await object<any>(api, "cue_list", stored.id);
	expect(final.body.cues.map((cue: any) => cue.number)).toEqual([
		1, 2, 3, 4, 5, 6, 7,
	]);
	expect(
		final.body.cues.map((cue: any) =>
			cue.group_changes.map((change: any) => [
				change.group_id,
				rounded(change.value.value),
			]),
		),
	).toEqual([
		[["1", 1]],
		[["2", 1]],
		[["3", 1]],
		[["1", 0.5]],
		[["2", 0.2]],
		[["3", 0.3]],
		[["1", 0.4]],
	]);
	state.completed = true;
};
