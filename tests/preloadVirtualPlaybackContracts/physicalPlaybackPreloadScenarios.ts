import {
	type BenchContractContext,
	type BenchUiContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "../../apps/control-ui/e2e/bench/programmerPreloadLifecycle";
import { clearProgrammerValues } from "../../apps/control-ui/e2e/bench/programmerValues";
import { programmer } from "../support/catalog";
import {
	activePlayback,
	longPressPreload,
	openPlaybackMode,
	type PlaybackSpec,
	type PreloadPlaybackPairState,
	playbackCard,
	playbacks,
	poolAction,
	prepare,
	setCaptureMask,
	summarizePlaybackState,
	timestampMillis,
	visualizationLevel,
} from "./support";

const preload002Scenario: PairedScenario<PreloadPlaybackPairState> = {
	id: "PRELOAD-002",
	title:
		"physical-playback-only Preload preserves the seven ordered action verbs",
	arrange: async ({ api, bench }, surface) => {
		const actions = ["toggle", "go", "go_minus", "off", "on", "temp"] as const;
		const specs = actions.map(
			(action, index): PlaybackSpec => ({
				number: index + 1,
				fixture: index + 3,
				levels: [0.3, 0.7],
				name: `Paired ${action}`,
				buttons: [action, "none", "none"],
				buttonCount: 1,
				hasFader: false,
			}),
		);
		const prepared = await prepare(
			api,
			bench,
			`preload-002-paired-${surface}`,
			specs,
			Object.fromEntries(specs.map((spec, index) => [index + 1, spec.number])),
		);
		await setCaptureMask(api, false, true, false, 2_000, 7_000);
		await poolAction(api, 3, "go");
		await poolAction(api, 3, "go");
		await poolAction(api, 4, "go");
		return prepared;
	},
	api: async ({ api, bench }, state) => {
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		for (const [number, action] of [
			[1, "toggle"],
			[2, "go"],
			[3, "go-minus"],
			[4, "off"],
			[5, "on"],
			[6, "temp-on"],
			[6, "temp-off"],
		] as const)
			await poolAction(api, number, action, { surface: "physical" });
		state.pendingActions = (await programmer(api)).preload_playback_pending.map(
			(entry: any) => entry.action,
		);
		state.applicationTimestamp = (
			await goProgrammerPreload(api, {
				surface: "api",
				showId: state.showId,
			})
		).commit!.committedAt;
		await bench.tick(2_000);
		state.committedState = summarizePlaybackState(
			await playbacks(api),
			[1, 2, 3, 4, 5, 6],
		);
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		state.releasedState = summarizePlaybackState(
			await playbacks(api),
			[1, 2, 3, 4, 5, 6],
		);
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
		for (const [slot, label] of [
			[1, "TOGGLE"],
			[2, "GO +"],
			[3, "GO −"],
			[4, "OFF"],
			[5, "ON"],
			[6, "TEMP"],
			[6, "TEMP"],
		] as const)
			await playbackCard(page, slot)
				.getByRole("button", { name: label, exact: true })
				.click();
		state.pendingActions = (await programmer(api)).preload_playback_pending.map(
			(entry: any) => entry.action,
		);
		await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
		state.applicationTimestamp = (await activePlayback(api, 1))?.activated_at;
		await bench.tick(2_000);
		state.committedState = summarizePlaybackState(
			await playbacks(api),
			[1, 2, 3, 4, 5, 6],
		);
		await longPressPreload(page);
		state.releasedState = summarizePlaybackState(
			await playbacks(api),
			[1, 2, 3, 4, 5, 6],
		);
	},
	assert: async ({ api }, state) => {
		expect(state.pendingActions).toEqual([
			"toggle",
			"go",
			"go-minus",
			"off",
			"on",
			"temp-on",
			"temp-off",
		]);
		expect(state.applicationTimestamp).toEqual(expect.any(String));
		expect(await activePlayback(api, 1)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(await activePlayback(api, 2)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(timestampMillis((await activePlayback(api, 1))?.activated_at)).toBe(
			timestampMillis(state.applicationTimestamp),
		);
		expect(timestampMillis((await activePlayback(api, 2))?.activated_at)).toBe(
			timestampMillis(state.applicationTimestamp),
		);
		expect(await activePlayback(api, 3)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
		expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
		expect((await activePlayback(api, 6))?.temporary_active ?? false).toBe(
			false,
		);
		expect(state.releasedState).toEqual(state.committedState);
	},
};

const preload002ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const specs: PlaybackSpec[] = Array.from({ length: 9 }, (_, index) => ({
		number: index + 1,
		fixture: index + 3,
		levels: [0.25, 0.5, 0.75, 1],
		name: `Physical ${index + 1}`,
		buttons: index === 7 ? ["go", "go_minus", "flash"] : ["go", "none", "none"],
		buttonCount: index === 7 ? 3 : 1,
		hasFader: index === 7,
	}));
	const prepared = await prepare(
		api,
		bench,
		"preload-002-physical",
		specs,
		Object.fromEntries(
			specs.slice(0, 8).map((spec, index) => [index + 1, spec.number]),
		),
	);
	await setCaptureMask(api, false, true, false, 2_000, 7_000);
	await poolAction(api, 2, "go");
	await poolAction(api, 2, "go");
	await poolAction(api, 3, "go");
	await poolAction(api, 3, "off");
	await poolAction(api, 4, "go");
	await poolAction(api, 5, "go");
	await poolAction(api, 5, "off");
	await poolAction(api, 7, "temp-on");
	await poolAction(api, 9, "go");

	await enterProgrammerPreload(api, {
		surface: "api",
		showId: prepared.showId,
	});
	await api.executeCommandLine("GROUP 1 AT 40");
	expect((await programmer(api)).preload_group_pending).toEqual({});
	expect((await programmer(api)).group_values["1"]).toBeDefined();
	// The disabled-domain assertion is complete; clear its live value so the playback timing
	// checkpoint below measures only the queued GO result.
	await clearProgrammerValues(api, {
		surface: "api",
		showId: prepared.showId,
	});
	const verbs = [
		[1, "go"],
		[2, "go-minus"],
		[3, "on"],
		[4, "off"],
		[5, "toggle"],
		[6, "temp-on"],
		[7, "temp-off"],
	] as const;
	for (const [number, action] of verbs)
		await poolAction(api, number, action, { surface: "physical" });
	expect(
		(await programmer(api)).preload_playback_pending.map(
			(entry: any) => entry.action,
		),
	).toEqual(verbs.map(([, action]) => action));

	await poolAction(api, 8, "button", {
		button: 3,
		pressed: true,
		surface: "physical",
	});
	expect((await activePlayback(api, 8))?.flash).toBe(true);
	await poolAction(api, 8, "button", {
		button: 3,
		pressed: false,
		surface: "physical",
	});
	await poolAction(api, 8, "master", { value: 0.4, surface: "physical" });
	expect(await activePlayback(api, 8)).toMatchObject({ enabled: true });
	expect((await activePlayback(api, 8))?.fader_position).toBeCloseTo(0.4, 5);
	expect(
		(await programmer(api)).preload_playback_pending.map(
			(entry: any) => entry.action,
		),
	).toEqual(verbs.map(([, action]) => action));

	await poolAction(api, 9, "go", { surface: "physical" });
	await poolAction(api, 9, "go", { surface: "physical" });
	await poolAction(api, 9, "go", { surface: "virtual" });
	expect((await activePlayback(api, 9))?.current_cue_number).toBe(2);
	expect(
		(await programmer(api)).preload_playback_pending
			.slice(-2)
			.map((entry: any) => entry.action),
	).toEqual(["go", "go"]);

	await bench.tick(100);
	const committed = (
		await goProgrammerPreload(api, {
			surface: "api",
			showId: prepared.showId,
		})
	).commit!;
	expect(committed.programmerFadeMillis).toBe(2_000);
	expect(committed.executed.map((entry) => entry.action)).toEqual([
		"go",
		"back",
		"on",
		"off",
		"toggle",
		"temporary_on",
		"temporary_off",
		"go",
		"go",
	]);
	expect(timestampMillis((await activePlayback(api, 1))?.activated_at)).toBe(
		timestampMillis(committed.committedAt),
	);
	await bench.tick(1_000);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(
		0.125,
		2,
	);
	await bench.tick(1_000);
	expect(await activePlayback(api, 1)).toMatchObject({
		enabled: true,
		current_cue_number: 1,
	});
	expect(await activePlayback(api, 2)).toMatchObject({
		enabled: true,
		current_cue_number: 1,
	});
	expect(await activePlayback(api, 3)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 6)).toMatchObject({
		temporary_active: true,
	});
	// Temp off against an otherwise inactive playback removes the transient runtime entry.
	expect(await activePlayback(api, 7)).toBeUndefined();
	expect(await activePlayback(api, 9)).toMatchObject({ current_cue_number: 4 });

	const playbackState = summarizePlaybackState(
		await playbacks(api),
		[1, 2, 3, 4, 5, 6, 7, 9],
	);
	await releaseProgrammerPreload(api, {
		surface: "api",
		showId: prepared.showId,
	});
	expect(
		summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6, 7, 9]),
	).toEqual(playbackState);
};

const preload002UiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	const actions = ["toggle", "go", "go_minus", "off", "on", "temp"] as const;
	const specs = actions.map(
		(action, index): PlaybackSpec => ({
			number: index + 1,
			fixture: index + 3,
			levels: [0.3, 0.7],
			name: `UI ${action}`,
			buttons:
				index === 0 ? [action, "flash", "none"] : [action, "none", "none"],
			buttonCount: index === 0 ? 2 : 1,
			hasFader: index === 0,
		}),
	);
	await prepare(
		api,
		bench,
		"preload-002-ui",
		specs,
		Object.fromEntries(specs.map((spec, index) => [index + 1, spec.number])),
	);
	await setCaptureMask(api, false, true, false, 2_000, 7_000);
	await poolAction(api, 3, "go");
	await poolAction(api, 3, "go");
	await poolAction(api, 4, "go");
	await poolAction(api, 5, "go");
	await poolAction(api, 5, "off");
	await desk.open(bench.baseUrl);
	await openPlaybackMode(page);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
	await desk.recordStep(
		"QUEUE PHYSICAL VERBS",
		"Toggle, GO, GO minus, Off, On, Temp press, and Temp release are retained in operator order.",
	);
	for (const [slot, label] of [
		[1, "TOGGLE"],
		[2, "GO +"],
		[3, "GO −"],
		[4, "OFF"],
		[5, "ON"],
	] as const)
		await playbackCard(page, slot)
			.getByRole("button", { name: label, exact: true })
			.click();
	const temp = playbackCard(page, 6).getByRole("button", {
		name: "TEMP",
		exact: true,
	});
	await temp.click();
	await expect
		.poll(
			async () =>
				(await programmer(api)).preload_playback_pending.at(-1)?.action,
		)
		.toBe("temp-on");
	await temp.click();
	const pending = await programmer(api);
	expect(
		pending.preload_playback_pending.map((entry: any) => entry.action),
	).toEqual(["toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off"]);
	const flash = playbackCard(page, 1).getByRole("button", {
		name: "FLASH",
		exact: true,
	});
	await flash.hover();
	await page.mouse.down();
	await page.mouse.up();
	await playbackCard(page, 1)
		.getByRole("slider", { name: "Master" })
		.fill("40");
	expect((await programmer(api)).preload_playback_pending).toHaveLength(7);
	await expect(
		page.getByTitle(
			/Pending Preload: .*TOGGLE 1.*GO 2.*GO MINUS 3.*OFF 4.*ON 5.*TEMP ON 6.*TEMP OFF 6/,
		),
	).toBeVisible();
	await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
	await bench.tick(2_000);
	expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
};

export function registerPhysicalPlaybackPreloadScenarios(): void {
	pairedScenario(preload002Scenario);
	test(
		"PRELOAD-002 @supplemental › API queue ordering, live Flash/fader exclusions, and timing",
		preload002ApiSupplement,
	);
	test(
		"PRELOAD-002 @supplemental-ui › physical controls expose all verbs and live exclusions",
		preload002UiSupplement,
	);
}
