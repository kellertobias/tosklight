import { expect } from "../bench/core/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "../support/catalog";
import {
	fixtureCue,
	installPlaybackSequence,
	playbackState,
	registerPairedCueScenario,
	runtime,
	setSequenceMasterFade,
	slot,
	visualizationLevel,
} from "./support";

registerPairedCueScenario<{ completed: boolean }>({
	id: "CUE-003",
	title:
		"GO, pause, resume, back, and release use exact application-time boundaries",
	surfaces: ["api"],
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		await loadCanonicalCopy(api, bench, "cue-003-exact-timing", "compact-rig");
		await setSequenceMasterFade(api, 0);
		const fixture = (await fixtureIdsByNumber(api))[1];
		const installed = await installPlaybackSequence(api, 1, [
			fixtureCue(1, [[fixture, "intensity", 0]], { fade_millis: 0 }),
			fixtureCue(2, [[fixture, "intensity", 1]], { fade_millis: 4_000 }),
		]);
		await api.playbackNumberAction(1, "go", {});
		await api.playbackNumberAction(1, "go", {});
		expect(slot(await bench.tick(0), 1)).toBe(0);
		expect(slot(await bench.tick(2_000), 1)).toBe(128);
		expect(slot(await bench.tick(2_000), 1)).toBe(255);

		await api.playbackNumberAction(1, "off", {});
		await api.playbackNumberAction(1, "go", {});
		await api.playbackNumberAction(1, "go", {});
		expect(slot(await bench.tick(1_000), 1)).toBe(64);
		await api.cueListPlaybackAction(installed.id, "pause", {});
		const paused = await runtime(api, 1);
		expect(paused.paused).toBe(true);
		expect(slot(await bench.tick(10_000), 1)).toBe(64);
		expect((await runtime(api, 1)).activated_at).toBe(paused.activated_at);
		await api.cueListPlaybackAction(installed.id, "go", {});
		expect((await runtime(api, 1)).paused).toBe(false);
		expect(slot(await bench.tick(3_000), 1)).toBe(255);
		await api.cueListPlaybackAction(installed.id, "back", {});
		expect((await runtime(api, 1)).current_cue_number).toBe(1);
		expect(slot(await bench.tick(0), 1)).toBe(0);
		await api.cueListPlaybackAction(installed.id, "release", {});
		expect((await playbackState(api)).active).toHaveLength(0);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await loadCanonicalCopy(api, bench, "cue-003-visible-pause", "compact-rig");
		await setSequenceMasterFade(api, 0);
		const fixture = (await fixtureIdsByNumber(api))[1];
		await installPlaybackSequence(api, 1, [
			fixtureCue(1, [[fixture, "intensity", 0]]),
			fixtureCue(2, [[fixture, "intensity", 1]], { fade_millis: 4_000 }),
		]);
		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		const card = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Playback 1" });
		await page.getByRole("button", { name: "SET", exact: true }).click();
		await card
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		const configuration = page.getByRole("dialog", {
			name: "Playback Configuration",
		});
		await expect(configuration).toBeVisible();
		await configuration
			.getByRole("button", { name: "Layout", exact: true })
			.click();
		await configuration
			.locator(".ui-form-field")
			.filter({ hasText: /^\s*Bottom button/ })
			.locator(".ui-select-trigger")
			.click();
		await page
			.getByRole("dialog", { name: "Choose Bottom button function" })
			.getByRole("button", { name: /^Pause/ })
			.click();
		await configuration
			.getByRole("button", { name: "Apply", exact: true })
			.click();
		await expect(configuration).toBeHidden();
		await expect
			.poll(async () => (await object<any>(api, "playback", "1")).body.buttons)
			.toEqual(["go_minus", "go", "pause"]);

		await card.getByRole("button", { name: "GO +", exact: true }).click();
		await card.getByRole("button", { name: "GO +", exact: true }).click();
		expect(slot(await bench.tick(1_000), 1)).toBe(64);
		await card.getByRole("button", { name: "PAUSE", exact: true }).click();
		await expect
			.poll(async () => runtime(api, 1))
			.toMatchObject({ current_cue_number: 2, paused: true });
		await expect(
			card.getByRole("button", { name: "RESUME", exact: true }),
		).toHaveClass(/playback-button-active/);
		expect(slot(await bench.tick(10_000), 1)).toBe(64);
		await card.getByRole("button", { name: "RESUME", exact: true }).click();
		await expect
			.poll(async () => runtime(api, 1))
			.toMatchObject({ current_cue_number: 2, paused: false });
		expect(slot(await bench.tick(3_000), 1)).toBe(255);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

registerPairedCueScenario<{ completed: boolean }>({
	id: "CUE-004",
	title:
		"per-value timing overrides Cue fallback and Force Cue Timing is reversible",
	surfaces: ["api"],
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		await loadCanonicalCopy(api, bench, "cue-004-value-timing", "compact-rig");
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const installed = await installPlaybackSequence(api, 1, [
			fixtureCue(
				1,
				[
					[fixtures[1], "intensity", 0.5, { fade_millis: 2_000 }],
					[fixtures[2], "intensity", 0.7, { fade_millis: 4_000 }],
					[fixtures[3], "intensity", 0.6],
					[
						fixtures[4],
						"intensity",
						0.8,
						{ fade_millis: 1_000, delay_millis: 1_000 },
					],
				],
				{ fade_millis: 3_000, delay_millis: 500 },
			),
		]);

		await api.playbackNumberAction(1, "go", {});
		await bench.tick(499);
		expect(await visualizationLevel(api, fixtures[1], "intensity")).toBe(0);
		expect(await visualizationLevel(api, fixtures[4], "intensity")).toBe(0);
		await bench.tick(501);
		expect(await visualizationLevel(api, fixtures[4], "intensity")).toBe(0);
		await bench.tick(1_000);
		expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo(
			0.8,
			5,
		);
		await bench.tick(500);
		expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(
			0.5,
			5,
		);
		await bench.tick(1_000);
		expect(await visualizationLevel(api, fixtures[3], "intensity")).toBeCloseTo(
			0.6,
			5,
		);
		await bench.tick(1_000);
		expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(
			0.7,
			5,
		);

		await api.playbackNumberAction(1, "off", {});
		let stored = await object<any>(api, "cue_list", installed.id);
		const timingBytes = JSON.stringify(stored.body.cues[0].changes);
		await putObject(
			api,
			"cue_list",
			installed.id,
			{ ...stored.body, force_cue_timing: true },
			stored.revision,
		);
		await api.playbackNumberAction(1, "go", {});
		await bench.tick(2_500);
		expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(
			1 / 3,
			2,
		);
		expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo(
			(0.8 * 2) / 3,
			2,
		);
		await bench.tick(1_000);
		for (const [fixture, target] of [
			[fixtures[1], 0.5],
			[fixtures[2], 0.7],
			[fixtures[3], 0.6],
			[fixtures[4], 0.8],
		] as const)
			expect(await visualizationLevel(api, fixture, "intensity")).toBeCloseTo(
				target,
				5,
			);
		stored = await object<any>(api, "cue_list", installed.id);
		expect(JSON.stringify(stored.body.cues[0].changes)).toBe(timingBytes);
		await putObject(
			api,
			"cue_list",
			installed.id,
			{ ...stored.body, force_cue_timing: false },
			stored.revision,
		);
		await api.playbackNumberAction(1, "off", {});
		await api.playbackNumberAction(1, "go", {});
		await bench.tick(2_000);
		expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(
			0.375,
			5,
		);
		expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo(
			0.8,
			5,
		);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
