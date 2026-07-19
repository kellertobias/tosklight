import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	armSet,
	authoritativeMasterObservation,
	controls,
	definition,
	expectConfigurationModal,
	installPlaybacks,
	openPlaybackMode,
	playbackAt,
	playbackCard,
	playbackSlider,
	poolAction,
	prepareShow,
	pressButton,
	programmer,
	selectTrigger,
	setSpeedRates,
} from "./helpers";
import type { PreparedShow } from "./models";
import { runPbk006ActionMatrixScenario } from "./pbk006ActionMatrix";

export function registerPbk006PairedScenario(): void {
	pairedScenario<PreparedShow>({
		id: "PBK-006",
		title:
			"specialized layouts control their authoritative Speed, Group, Grand, and Fade masters",
		arrange: async ({ api, bench }, surface) => {
			const prepared = await prepareShow(
				api,
				bench,
				`pbk-006-paired-${surface}`,
				"default-stage",
			);
			await setSpeedRates(api, [120, 96, 72, 60, 48]);
			for (const [group, bpm] of [
				[1, 120],
				[2, 96],
				[3, 72],
				[4, 60],
				[5, 48],
			] as const)
				await api.executeLegacyCommandLine(`SPD GRP ${group} AT ${bpm}`);
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
							color: "#8b5cf6",
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
			return prepared;
		},
		api: async ({ api }) => {
			await pressButton(api, 61, 1);
			await poolAction(api, 61, "master", { value: 0.5 });
			await poolAction(api, 62, "master", { value: 0.4 });
			await pressButton(api, 62, 1);
			await poolAction(api, 63, "master", { value: 0.3 });
			await pressButton(api, 63, 1);
			await poolAction(api, 64, "master", { value: 0.25 });
			await poolAction(api, 65, "master", { value: 0.25 });
		},
		ui: async ({ bench, desk, page }) => {
			await desk.open(bench.baseUrl);
			await openPlaybackMode(page);
			await playbackCard(page, 1)
				.getByRole("button", { name: "DOUBLE", exact: true })
				.click();
			await playbackSlider(page, 1).fill("50");
			await playbackSlider(page, 2).fill("40");
			await playbackCard(page, 2)
				.getByRole("button", { name: "SELECT", exact: true })
				.click();
			await playbackSlider(page, 3).fill("30");
			await playbackCard(page, 3)
				.getByRole("button", { name: "BLACKOUT", exact: true })
				.click();
			await playbackSlider(page, 4).fill("25");
			await playbackSlider(page, 5).fill("25");
		},
		assert: async ({ api }) => {
			await expect
				.poll(async () => authoritativeMasterObservation(await controls(api)))
				.toEqual({
					speed: { manualBpm: 240, effectiveBpm: 120, paused: false },
					neighborBpms: [96, 72, 60, 48],
					group: { master: 0.4, flashLevel: 0 },
					grand: {
						level: 0.3,
						effectiveLevel: 0.3,
						blackout: true,
						dynamicsPaused: false,
					},
					programmerFadeMillis: 5_000,
					cueFadeMillis: 15_000,
				});
			await expect
				.poll(async () => (await programmer(api)).selection_expression)
				.toMatchObject({ type: "live_group", group_id: "1" });
			expect((await playbackAt(api, 1, 1)).body).toMatchObject({
				target: { type: "speed_group", group: "A" },
				buttons: ["double", "half", "learn"],
				fader: "learned_percentage",
				color: "#8b5cf6",
			});
			expect((await playbackAt(api, 1, 2)).body.target).toEqual({
				type: "group",
				group_id: "1",
			});
			expect((await playbackAt(api, 1, 3)).body.target).toEqual({
				type: "grand_master",
			});
			expect((await playbackAt(api, 1, 4)).body).toMatchObject({
				target: { type: "programmer_fade" },
				buttons: ["double", "half", "off"],
			});
			expect((await playbackAt(api, 1, 5)).body).toMatchObject({
				target: { type: "cue_fade" },
				buttons: ["double", "half", "off"],
			});
		},
	});
}

export function registerPbk006ActionMatrixScenario(): void {
	test(
		"PBK-006 @supplemental › specialized layouts cover every action and exact fader checkpoint",
		runPbk006ActionMatrixScenario,
	);
}

export function registerPbk006UiScenario(): void {
	test("PBK-006 @supplemental-ui › specialized controls render fixed layouts and detailed feedback", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await prepareShow(api, bench, "pbk-006-ui", "default-stage");
		await setSpeedRates(api, [120, 96, 72, 60, 48]);
		await installPlaybacks(
			api,
			[
				definition(
					66,
					"Speed A",
					{ type: "speed_group", group: "A" },
					{
						buttons: ["double", "half", "learn"],
						fader: "learned_percentage",
						color: "#8b5cf6",
					},
				),
				definition(
					67,
					"Group 1",
					{ type: "group", group_id: "1" },
					{ buttons: ["select", "select_dereferenced", "flash"] },
				),
				definition(
					68,
					"Grand",
					{ type: "grand_master" },
					{ buttons: ["blackout", "pause_dynamics", "flash"] },
				),
				definition(
					69,
					"Programmer Fade",
					{ type: "programmer_fade" },
					{ buttons: ["double", "half", "off"] },
				),
				definition(
					70,
					"Cue Fade",
					{ type: "cue_fade" },
					{ buttons: ["double", "half", "off"] },
				),
			],
			{ 1: 66, 2: 67, 3: 68, 4: 69, 5: 70 },
		);
		await desk.open(bench.baseUrl);
		await setSpeedRates(api, [120, 96, 72, 60, 48]);
		await page.reload();
		await openPlaybackMode(page);
		await expect
			.poll(async () => (await controls(api)).speed_groups[0].effective_bpm)
			.toBeCloseTo(120, 3);
		await expect(playbackCard(page, 1)).toContainText("120 BPM");
		await expect(playbackCard(page, 1)).toHaveCSS(
			"--playback-color",
			"#8b5cf6",
		);
		const threeButtonLayout = playbackCard(page, 1).locator(
			".vertical-touch-fader-actions > .ui-button",
		);
		await expect(threeButtonLayout).toHaveCount(3);
		const [firstButtonBox, secondButtonBox, bottomButtonBox] =
			await Promise.all(
				[0, 1, 2].map((index) => threeButtonLayout.nth(index).boundingBox()),
			);
		expect(firstButtonBox).not.toBeNull();
		expect(secondButtonBox).not.toBeNull();
		expect(bottomButtonBox).not.toBeNull();
		expect(Math.abs(firstButtonBox!.y - secondButtonBox!.y)).toBeLessThan(2);
		expect(bottomButtonBox!.y).toBeGreaterThan(
			firstButtonBox!.y + firstButtonBox!.height,
		);
		expect(bottomButtonBox!.width).toBeGreaterThan(firstButtonBox!.width * 1.9);
		await playbackCard(page, 1)
			.getByRole("button", { name: "DOUBLE", exact: true })
			.click();
		await expect
			.poll(async () => (await controls(api)).speed_groups[0].manual_bpm)
			.toBe(240);
		await expect(playbackCard(page, 1)).toContainText("240 BPM");

		await poolAction(api, 67, "master", { value: 0.4 });
		await expect(playbackCard(page, 2)).toContainText("40% master");
		await expect(
			playbackCard(page, 2).getByRole("slider", { name: "Group master" }),
		).toHaveValue("40");
		await poolAction(api, 68, "master", { value: 0.3 });
		await expect(
			playbackCard(page, 3).getByRole("slider", { name: "Grand Master" }),
		).toHaveValue("30");
		await poolAction(api, 69, "master", { value: 0.25 });
		await poolAction(api, 70, "master", { value: 0.25 });
		await expect(playbackCard(page, 4)).toContainText("5.0 s");
		await expect(playbackCard(page, 5)).toContainText("15.0 s");
		for (const slot of [4, 5]) {
			await expect(
				playbackCard(page, slot).getByRole("button", {
					name: "DOUBLE",
					exact: true,
				}),
			).toBeVisible();
			await expect(
				playbackCard(page, slot).getByRole("button", {
					name: "HALF",
					exact: true,
				}),
			).toBeVisible();
			await expect(
				playbackCard(page, slot).getByRole("button", {
					name: "OFF",
					exact: true,
				}),
			).toBeVisible();
		}

		await armSet(page);
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		const modal = await expectConfigurationModal(page, 1, 1);
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(selectTrigger(modal, "Top button")).toContainText("Double");
		await expect(selectTrigger(modal, "Middle button")).toContainText("Half");
		await expect(selectTrigger(modal, "Bottom button")).toContainText("Learn");
		await expect(selectTrigger(modal, "Fader")).toContainText(
			"Learned-speed percentage",
		);
		await modal
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
	});
}

export function registerPbk006OscScenario(): void {
	test("PBK-006 @osc › external controls and LED/fader/action feedback share the authoritative master state", async ({
		api,
		bench,
	}) => {
		await prepareShow(api, bench, "pbk-006-osc", "default-stage");
		await setSpeedRates(api, [120, 96, 72, 60, 48]);
		await installPlaybacks(
			api,
			[
				definition(
					71,
					"OSC Speed",
					{ type: "speed_group", group: "A" },
					{
						buttons: ["double", "half", "learn"],
						fader: "learned_percentage",
						color: "#8b5cf6",
					},
				),
			],
			{ 1: 71 },
		);
		const hardware = await bench.osc();
		const alias = api.session!.desk.osc_alias;
		try {
			await hardware.subscribe(`pbk-006-${crypto.randomUUID()}`, alias);
			let mark = hardware.mark();
			await bench.tick(0);
			const fader = await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/page-playback/1/fader`,
			);
			expect(fader.arguments[0]).toBeCloseTo(1, 4);
			const led = await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/page-playback/1/button/1`,
			);
			expect(led.arguments.slice(0, 3)).toEqual(
				expect.arrayContaining([
					expect.closeTo((0x8b / 255) * 0.35, 4),
					expect.closeTo((0x5c / 255) * 0.35, 4),
					expect.closeTo((0xf6 / 255) * 0.35, 4),
				]),
			);
			expect(led.arguments[3]).toBe("off");
			expect(
				(
					await hardware.expectAfter(
						mark,
						`/light/${alias}/feedback/page-playback/1/button/1/action`,
					)
				).arguments,
			).toEqual(["double"]);

			mark = hardware.mark();
			await hardware.send(`/light/${alias}/page-playback/1/button/1`, [true]);
			await expect
				.poll(async () => (await controls(api)).speed_groups[0].manual_bpm)
				.toBe(240);
			await bench.tick(0);
			const speedFeedbackAddress = `/light/${alias}/feedback/speed-group/1`;
			await expect
				.poll(() =>
					hardware.messages
						.slice(mark)
						.some(
							(message) =>
								message.address === speedFeedbackAddress &&
								message.arguments[0] === 240,
						),
				)
				.toBe(true);

			mark = hardware.mark();
			await hardware.send(`/light/${alias}/page-playback/1/fader`, [0.5]);
			await expect
				.poll(async () => (await controls(api)).speed_groups[0].effective_bpm)
				.toBeCloseTo(120, 3);
			await bench.tick(0);
			const faderFeedbackAddress = `/light/${alias}/feedback/page-playback/1/fader`;
			await expect
				.poll(() =>
					hardware.messages
						.slice(mark)
						.some(
							(message) =>
								message.address === faderFeedbackAddress &&
								typeof message.arguments[0] === "number" &&
								Math.abs(message.arguments[0] - 0.5) < 0.0001,
						),
				)
				.toBe(true);
		} finally {
			await hardware.close();
		}
	});
}
