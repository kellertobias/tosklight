import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	activePlayback,
	createCueList,
	definition,
	installPlaybacks,
	object,
	openPlaybackMode,
	playbackAt,
	playbackCard,
	playbackSlider,
	playbackSnapshot,
	poolAction,
	prepareShow,
	serializedCueTimings,
	updatePlayback,
	visualizationLevel,
	xfadeObservation,
} from "./helpers";
import type { PlaybackCheckpoint, PreparedShow } from "./models";

type Pbk004State = PreparedShow & {
	timings: string;
	checkpoints: PlaybackCheckpoint[];
};

export function registerPbk004PairedScenario(): void {
	pairedScenario<Pbk004State>({
		id: "PBK-004",
		title:
			"X-fade travel advances one Cue and preserves manual direction and timing",
		arrange: async ({ api, bench }, surface) => {
			const prepared = await prepareShow(
				api,
				bench,
				`pbk-004-paired-${surface}`,
				"compact-rig",
				[0, 1, 0.5],
				30_000,
				10_000,
			);
			await installPlaybacks(
				api,
				[
					definition(
						47,
						"Manual Crossfade",
						{ type: "cue_list", cue_list_id: prepared.cueListId },
						{ fader: "x_fade" },
					),
				],
				{ 1: 47 },
			);
			await poolAction(api, 47, "on");
			return {
				...prepared,
				timings: serializedCueTimings(
					await object<any>(api, "cue_list", prepared.cueListId),
				),
				checkpoints: [],
			};
		},
		api: async ({ api, bench }, state) => {
			await poolAction(api, 47, "master", { value: 0.25 });
			await bench.tick(0);
			state.checkpoints.push(
				xfadeObservation(
					await activePlayback(api, 47),
					await visualizationLevel(api, state.fixtures[1]),
				),
			);
			await poolAction(api, 47, "master", { value: 1 });
			await bench.tick(0);
			state.checkpoints.push(
				xfadeObservation(
					await activePlayback(api, 47),
					await visualizationLevel(api, state.fixtures[1]),
				),
			);
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			await openPlaybackMode(page);
			const slider = playbackSlider(page, 1);
			await slider.fill("25");
			await expect
				.poll(async () => (await activePlayback(api, 47)).manual_xfade_progress)
				.toBeCloseTo(0.25, 3);
			await bench.tick(0);
			state.checkpoints.push(
				xfadeObservation(
					await activePlayback(api, 47),
					await visualizationLevel(api, state.fixtures[1]),
				),
			);
			await slider.fill("100");
			await expect
				.poll(async () => (await activePlayback(api, 47)).current_cue_number)
				.toBe(2);
			await bench.tick(0);
			state.checkpoints.push(
				xfadeObservation(
					await activePlayback(api, 47),
					await visualizationLevel(api, state.fixtures[1]),
				),
			);
		},
		assert: async ({ api }, state) => {
			expect(state.checkpoints[0]).toEqual({
				cue: 1,
				position: 0.25,
				progress: 0.25,
				direction: "towards_high",
				intensity: 0.25,
			});
			expect(state.checkpoints[1]).toMatchObject({
				cue: 2,
				position: 1,
				direction: "towards_low",
				intensity: 1,
			});
			expect(await activePlayback(api, 47)).toMatchObject({
				current_cue_number: 2,
				manual_xfade_position: 1,
				manual_xfade_direction: "towards_low",
			});
			expect((await playbackAt(api, 1, 1)).body.fader).toBe("x_fade");
			expect(
				serializedCueTimings(
					await object<any>(api, "cue_list", state.cueListId),
				),
			).toBe(state.timings);
		},
	});
}

export function registerPbk004OwnershipScenario(): void {
	test("PBK-004 @supplemental › Master, bidirectional X-fade, and Temp retain distinct ownership", async ({
		api,
		bench,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-004-faders",
			"compact-rig",
			[0, 1, 0.5],
			30_000,
			10_000,
		);
		await installPlaybacks(
			api,
			[
				definition(45, "Fader Modes", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
			],
			{ 1: 45 },
		);
		await poolAction(api, 45, "on");
		for (const level of [0, 0.5, 1]) {
			await poolAction(api, 45, "master", { value: level });
			expect(await activePlayback(api, 45)).toMatchObject({
				current_cue_number: 1,
				master: level,
				fader_position: level,
			});
		}

		await updatePlayback(api, 1, (current) => ({
			...current,
			fader: "x_fade",
		}));
		const timings = JSON.stringify(
			(await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map(
				(cue: any) => [cue.fade_millis, cue.delay_millis],
			),
		);
		for (const position of [0.25, 0.5, 0.75]) {
			await poolAction(api, 45, "master", { value: position });
			expect(await activePlayback(api, 45)).toMatchObject({
				manual_xfade_position: position,
				manual_xfade_progress: position,
				manual_xfade_direction: "towards_high",
				current_cue_number: 1,
			});
		}
		await poolAction(api, 45, "master", { value: 1 });
		expect(await activePlayback(api, 45)).toMatchObject({
			current_cue_number: 2,
			manual_xfade_direction: "towards_low",
			manual_xfade_position: 1,
		});
		await poolAction(api, 45, "master", { value: 1 });
		expect((await activePlayback(api, 45)).current_cue_number).toBe(2);
		for (const position of [0.75, 0.5, 0.25]) {
			await poolAction(api, 45, "master", { value: position });
			expect(await activePlayback(api, 45)).toMatchObject({
				manual_xfade_position: position,
				manual_xfade_progress: 1 - position,
				manual_xfade_direction: "towards_low",
				current_cue_number: 2,
			});
		}
		await poolAction(api, 45, "master", { value: 0 });
		expect(await activePlayback(api, 45)).toMatchObject({
			current_cue_number: 3,
			manual_xfade_direction: "towards_high",
			manual_xfade_position: 0,
		});
		expect(
			JSON.stringify(
				(await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map(
					(cue: any) => [cue.fade_millis, cue.delay_millis],
				),
			),
		).toBe(timings);

		const underneathId = await createCueList(
			api,
			prepared.fixtures,
			"Underlying",
			[0.3],
			0,
			0,
			[2],
		);
		await installPlaybacks(
			api,
			[
				{ ...(await playbackAt(api, 1, 1)).body },
				definition(46, "Underlying", {
					type: "cue_list",
					cue_list_id: underneathId,
				}),
			],
			{ 1: 45, 2: 46 },
		);
		await poolAction(api, 46, "on");
		const underneathBefore = await activePlayback(api, 46);
		await updatePlayback(api, 1, (current) => ({ ...current, fader: "temp" }));
		for (const level of [0.25, 0.5, 1]) {
			await poolAction(api, 45, "master", { value: level });
			expect(await activePlayback(api, 45)).toMatchObject({
				temporary_active: true,
				temporary_master: level,
			});
			expect(await activePlayback(api, 46)).toMatchObject({
				enabled: true,
				cue_index: underneathBefore.cue_index,
				activated_at: underneathBefore.activated_at,
			});
		}
		await poolAction(api, 45, "master", { value: 0 });
		expect(
			(await playbackSnapshot(api)).active.some(
				(item: any) => item.playback_number === 45 && item.temporary_active,
			),
		).toBe(false);
		expect(await activePlayback(api, 46)).toMatchObject({
			enabled: true,
			cue_index: underneathBefore.cue_index,
			activated_at: underneathBefore.activated_at,
		});

		await updatePlayback(api, 1, (current) => ({
			...current,
			fader: "x_fade",
		}));
		await poolAction(api, 45, "on");
		await poolAction(api, 45, "go-to", { cue_number: 1 });
		await poolAction(api, 45, "master", { value: 0.5 });
		const beforeReload = await activePlayback(api, 45);
		await bench.tick(0);
		expect(await activePlayback(api, 45)).toMatchObject({
			manual_xfade_direction: beforeReload.manual_xfade_direction,
			manual_xfade_position: 0.5,
			manual_xfade_progress: 0.5,
		});
	});
}

export function registerPbk004ReloadFeedbackScenario(): void {
	test("PBK-004 @supplemental-ui › X-fade progress survives browser reload as visible feedback", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-004-ui",
			"compact-rig",
			[0, 1, 0.5],
			30_000,
			10_000,
		);
		await installPlaybacks(
			api,
			[
				definition(
					47,
					"Manual Crossfade",
					{ type: "cue_list", cue_list_id: prepared.cueListId },
					{ fader: "x_fade" },
				),
			],
			{ 1: 47 },
		);
		await poolAction(api, 47, "on");
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		let slider = playbackCard(page, 1).getByRole("slider", { name: "X-fade" });
		await slider.fill("25");
		await expect
			.poll(async () => (await activePlayback(api, 47)).manual_xfade_progress)
			.toBeCloseTo(0.25, 3);
		await expect(playbackCard(page, 1)).toContainText("Cue 1 → 2 · 25%");
		await page.reload();
		await openPlaybackMode(page);
		await expect(playbackCard(page, 1)).toContainText("Cue 1 → 2 · 25%");
		slider = playbackCard(page, 1).getByRole("slider", { name: "X-fade" });
		await slider.fill("100");
		await expect
			.poll(async () => (await activePlayback(api, 47)).current_cue_number)
			.toBe(2);
		await expect(playbackCard(page, 1)).toContainText("Travel towards low");
	});
}
