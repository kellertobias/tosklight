import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	activePlayback,
	armSet,
	chooseSelect,
	definition,
	expectConfigurationModal,
	installPlaybacks,
	logicalDmx,
	object,
	openPlaybackMode,
	playbackAt,
	playbackCard,
	playbackSnapshot,
	poolAction,
	prepareShow,
	pressButton,
	programmer,
	setFirstButton,
} from "./helpers";
import type { PreparedShow } from "./models";

type Pbk003State = PreparedShow & {
	runtimeBeforeSelect?: any;
	dmxBeforeSelect?: number[];
};

export function registerPbk003PairedScenario(): void {
	pairedScenario<Pbk003State>({
		id: "PBK-003",
		title:
			"default navigation and remapped Select Contents dispatch one exact action",
		arrange: async ({ api, bench }, surface) => {
			const prepared = await prepareShow(
				api,
				bench,
				`pbk-003-paired-${surface}`,
				"compact-rig",
			);
			await installPlaybacks(
				api,
				[
					definition(43, "Action Matrix", {
						type: "cue_list",
						cue_list_id: prepared.cueListId,
					}),
				],
				{ 1: 43 },
			);
			return prepared;
		},
		api: async ({ api }, state) => {
			await pressButton(api, 43, 2);
			await pressButton(api, 43, 2);
			await pressButton(api, 43, 1);
			await setFirstButton(api, 1, "select_contents");
			state.runtimeBeforeSelect = await activePlayback(api, 43);
			state.dmxBeforeSelect = await logicalDmx(api);
			await pressButton(api, 43, 1);
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			await openPlaybackMode(page);
			const card = playbackCard(page, 1);
			await card.getByRole("button", { name: "GO +", exact: true }).click();
			await expect
				.poll(async () => (await activePlayback(api, 43)).current_cue_number)
				.toBe(1);
			await card.getByRole("button", { name: "GO +", exact: true }).click();
			await expect
				.poll(async () => (await activePlayback(api, 43)).current_cue_number)
				.toBe(2);
			await card.getByRole("button", { name: "GO −", exact: true }).click();
			await expect
				.poll(async () => (await activePlayback(api, 43)).current_cue_number)
				.toBe(1);
			await armSet(page);
			await card.getByRole("button", { name: "GO −", exact: true }).click();
			const modal = await expectConfigurationModal(page, 1, 1);
			await modal.getByRole("button", { name: "Layout", exact: true }).click();
			await chooseSelect(page, modal, "Top button", "Select contents");
			await modal.getByRole("button", { name: "Apply", exact: true }).click();
			await expect(modal).toBeHidden();
			state.runtimeBeforeSelect = await activePlayback(api, 43);
			state.dmxBeforeSelect = await logicalDmx(api);
			await card
				.getByRole("button", { name: "SELECT CONTENTS", exact: true })
				.click();
			await expect
				.poll(async () => (await programmer(api)).selection_expression?.type)
				.toBe("playback_contents");
		},
		assert: async ({ api }, state) => {
			expect((await playbackAt(api, 1, 1)).body.buttons).toEqual([
				"select_contents",
				"go",
				"flash",
			]);
			expect(await activePlayback(api, 43)).toEqual(state.runtimeBeforeSelect);
			expect(await logicalDmx(api)).toEqual(state.dmxBeforeSelect);
			const selected = await programmer(api);
			expect(selected.selection_expression).toEqual({
				type: "playback_contents",
				items: [
					{ type: "fixture", fixture_id: state.fixtures[1] },
					{ type: "fixture", fixture_id: state.fixtures[2] },
					{ type: "live_group", group_id: "3" },
				],
			});
			expect(selected.values).toEqual([]);
		},
	});
}

export function registerPbk003ActionMatrixScenario(): void {
	test("PBK-003 @supplemental › every Cuelist mapping preserves its distinct action semantics", async ({
		api,
		bench,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-003-actions",
			"compact-rig",
			[0.2, 0.8, 0.4],
			10_000,
			5_000,
		);
		await installPlaybacks(
			api,
			[
				definition(43, "Action Matrix", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
			],
			{ 1: 43 },
		);
		const timingBefore = (
			await object<any>(api, "cue_list", prepared.cueListId)
		).body.cues.map((cue: any) => ({
			fade_millis: cue.fade_millis,
			delay_millis: cue.delay_millis,
		}));

		await setFirstButton(api, 1, "go");
		await pressButton(api, 43);
		expect((await activePlayback(api, 43)).current_cue_number).toBe(1);
		await pressButton(api, 43);
		expect((await activePlayback(api, 43)).current_cue_number).toBe(2);
		await setFirstButton(api, 1, "go_minus");
		await pressButton(api, 43);
		expect((await activePlayback(api, 43)).current_cue_number).toBe(1);

		await setFirstButton(api, 1, "fast_forward");
		await pressButton(api, 43);
		expect(await activePlayback(api, 43)).toMatchObject({
			current_cue_number: 2,
			transition_timing_bypassed: true,
		});
		await setFirstButton(api, 1, "fast_rewind");
		await pressButton(api, 43);
		expect(await activePlayback(api, 43)).toMatchObject({
			current_cue_number: 1,
			transition_timing_bypassed: true,
		});
		expect(
			(await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map(
				(cue: any) => ({
					fade_millis: cue.fade_millis,
					delay_millis: cue.delay_millis,
				}),
			),
		).toEqual(timingBefore);

		await poolAction(api, 43, "master", { value: 0.25 });
		await setFirstButton(api, 1, "on");
		await pressButton(api, 43);
		expect(await activePlayback(api, 43)).toMatchObject({
			enabled: true,
			master: 1,
			fader_position: 0.25,
		});
		await setFirstButton(api, 1, "off");
		await pressButton(api, 43);
		expect(await activePlayback(api, 43)).toMatchObject({
			enabled: false,
			fader_position: 0.25,
			fader_pickup_required: true,
		});
		await poolAction(api, 43, "master", { value: 0.8 });
		expect(await activePlayback(api, 43)).toMatchObject({
			enabled: false,
			fader_pickup_required: true,
		});
		await poolAction(api, 43, "master", { value: 0 });
		await poolAction(api, 43, "master", { value: 0.6 });
		const recovered = await activePlayback(api, 43);
		expect(recovered).toMatchObject({
			enabled: true,
			fader_pickup_required: false,
		});
		expect(recovered.master).toBeCloseTo(0.6, 5);
		expect(recovered.fader_position).toBeCloseTo(0.6, 5);

		await setFirstButton(api, 1, "toggle");
		await pressButton(api, 43);
		expect((await activePlayback(api, 43)).enabled).toBe(false);
		await pressButton(api, 43);
		expect((await activePlayback(api, 43)).enabled).toBe(true);

		const beforeSelect = await activePlayback(api, 43);
		const dmxBeforeSelect = await logicalDmx(api);
		await setFirstButton(api, 1, "select");
		await pressButton(api, 43);
		expect((await playbackSnapshot(api)).selected_playback).toBe(43);
		expect(await activePlayback(api, 43)).toEqual(beforeSelect);
		expect(await logicalDmx(api)).toEqual(dmxBeforeSelect);

		await setFirstButton(api, 1, "select_contents");
		await pressButton(api, 43);
		const selected = await programmer(api);
		expect(selected.selection_expression).toMatchObject({
			type: "playback_contents",
			items: [
				{ type: "fixture", fixture_id: prepared.fixtures[1] },
				{ type: "fixture", fixture_id: prepared.fixtures[2] },
				{ type: "live_group", group_id: "3" },
			],
		});
		expect(selected.values).toHaveLength(0);
		expect((await activePlayback(api, 43)).current_cue_number).toBe(
			beforeSelect.current_cue_number,
		);
	});
}

export function registerPbk003PhysicalFeedbackScenario(): void {
	test("PBK-003 @supplemental-ui › physical controls expose default and remapped action feedback", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(api, bench, "pbk-003-ui", "compact-rig");
		await installPlaybacks(
			api,
			[
				definition(44, "UI Actions", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
			],
			{ 1: 44 },
		);
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		const card = playbackCard(page, 1);
		await expect(
			card.getByRole("button", { name: "GO −", exact: true }),
		).toBeVisible();
		await expect(
			card.getByRole("button", { name: "GO +", exact: true }),
		).toBeVisible();
		await expect(
			card.getByRole("button", { name: "FLASH", exact: true }),
		).toBeVisible();
		await card.getByRole("button", { name: "GO +", exact: true }).click();
		await expect
			.poll(async () => (await activePlayback(api, 44)).current_cue_number)
			.toBe(1);
		await card.getByRole("button", { name: "GO +", exact: true }).click();
		await expect
			.poll(async () => (await activePlayback(api, 44)).current_cue_number)
			.toBe(2);
		await card.getByRole("button", { name: "GO −", exact: true }).click();
		await expect
			.poll(async () => (await activePlayback(api, 44)).current_cue_number)
			.toBe(1);

		await armSet(page);
		await card.getByRole("button", { name: "GO −", exact: true }).click();
		const modal = await expectConfigurationModal(page, 1, 1);
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await chooseSelect(page, modal, "Top button", "Select contents");
		await modal.getByRole("button", { name: "Apply", exact: true }).click();
		await expect(modal).toBeHidden();
		const runtimeBefore = await activePlayback(api, 44);
		await card
			.getByRole("button", { name: "SELECT CONTENTS", exact: true })
			.click();
		await expect
			.poll(async () => (await programmer(api)).selection_expression?.type)
			.toBe("playback_contents");
		expect(await activePlayback(api, 44)).toEqual(runtimeBefore);
	});
}
