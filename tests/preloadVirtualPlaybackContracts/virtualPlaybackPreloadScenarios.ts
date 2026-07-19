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
import { programmer } from "../support/catalog";
import {
	activePlayback,
	addVirtualPlaybackPane,
	longPressPreload,
	openPlaybackMode,
	type PreloadVirtualPairState,
	playbackCard,
	playbackPendingObservation,
	poolAction,
	prepare,
	setCaptureMask,
	visualizationLevel,
} from "./support";

const preload004Scenario: PairedScenario<PreloadVirtualPairState> = {
	id: "PRELOAD-004",
	title: "virtual GO and TOGGLE alone remain pending and share Programmer Fade",
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepare(
			api,
			bench,
			`preload-004-paired-${surface}`,
			[
				{
					number: 44,
					fixture: 3,
					levels: [1],
					name: "Virtual GO",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 45,
					fixture: 4,
					levels: [0.8],
					name: "Virtual TOGGLE",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 46,
					fixture: 5,
					levels: [0.6],
					name: "Physical live",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 1: 44, 2: 45, 3: 46 },
		);
		await setCaptureMask(api, false, false, true, 2_500, 8_000);
		return prepared;
	},
	api: async ({ api, bench }, state) => {
		await api.command("preload.enter", {});
		// Keep the disabled-domain programmer proof on a distinct fixture: programmer priority is
		// intentionally higher than these Cuelists and must not mask the playback fade under test.
		await api.executeCommandLine("FIXTURE 1 AT 35");
		await poolAction(api, 46, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		await poolAction(api, 44, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		await poolAction(api, 45, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		state.pendingActions = playbackPendingObservation(await programmer(api));
		state.applicationTimestamp = (await api.command<any>("preload.go", {}))
			.payload!.application_timestamp;
		await bench.tick(2_500);
		await api.command("preload.release", {});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		const pane = await addVirtualPlaybackPane(page);
		await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
		await desk.command("1 AT 35", "F1 AT 35");
		await openPlaybackMode(page);
		await playbackCard(page, 3)
			.getByRole("button", { name: "GO +", exact: true })
			.click();
		await pane
			.getByRole("button", {
				name: /Virtual playback page 1 cell 1 Virtual GO/,
			})
			.click();
		await pane
			.getByRole("button", {
				name: /Virtual playback page 1 cell 2 Virtual TOGGLE/,
			})
			.click();
		state.pendingActions = playbackPendingObservation(await programmer(api));
		await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
		state.applicationTimestamp = (await activePlayback(api, 44))?.activated_at;
		await bench.tick(2_500);
		await longPressPreload(page);
	},
	assert: async ({ api }, state) => {
		expect(state.pendingActions).toEqual([
			[44, "go", "virtual"],
			[45, "toggle", "virtual"],
		]);
		expect(state.applicationTimestamp).toEqual(expect.any(String));
		expect(await activePlayback(api, 44)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
			activated_at: state.applicationTimestamp,
		});
		expect(await activePlayback(api, 45)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
			activated_at: state.applicationTimestamp,
		});
		expect(await activePlayback(api, 46)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		const finalProgrammer = await programmer(api);
		expect(finalProgrammer.preload_group_pending).toEqual({});
		expect(finalProgrammer.preload_group_active).toEqual({});
		expect(finalProgrammer.values).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					fixture_id: state.fixtures[1],
					attribute: "intensity",
				}),
			]),
		);
		expect(await visualizationLevel(api, state.fixtures[1])).toBeCloseTo(
			0.35,
			2,
		);
		expect(await visualizationLevel(api, state.fixtures[3])).toBeCloseTo(1, 2);
		expect(await visualizationLevel(api, state.fixtures[4])).toBeCloseTo(
			0.8,
			2,
		);
	},
};

const preload004ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const prepared = await prepare(
		api,
		bench,
		"preload-004-virtual-api",
		[
			{
				number: 41,
				fixture: 3,
				levels: [1],
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 42,
				fixture: 4,
				levels: [0.8],
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 43,
				fixture: 5,
				levels: [0.6],
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 1: 41, 2: 42, 3: 43 },
	);
	await setCaptureMask(api, false, false, true, 2_500, 8_000);
	await api.command("preload.enter", {});
	await api.executeCommandLine("GROUP 1 AT 35");
	await poolAction(api, 43, "button", {
		button: 1,
		pressed: true,
		surface: "physical",
	});
	await poolAction(api, 41, "button", {
		button: 1,
		pressed: true,
		surface: "virtual",
	});
	await poolAction(api, 42, "button", {
		button: 1,
		pressed: true,
		surface: "virtual",
	});
	const pending = await programmer(api);
	expect(pending.preload_group_pending).toEqual({});
	expect(
		pending.preload_playback_pending.map((entry: any) => [
			entry.action,
			entry.surface,
		]),
	).toEqual([
		["go", "virtual"],
		["toggle", "virtual"],
	]);
	expect(await activePlayback(api, 41)).toBeUndefined();
	expect(await activePlayback(api, 42)).toBeUndefined();
	expect(await activePlayback(api, 43)).toMatchObject({ enabled: true });
	// The disabled programmer domain was proven live above; remove it before measuring the
	// independently captured virtual playback transition.
	await api.command("programmer.clear", {});
	await bench.tick(100);
	const committed = (await api.command<any>("preload.go", {})).payload!;
	expect(
		committed.playback_actions.every(
			(entry: any) => entry.fallback_millis === 2_500,
		),
	).toBe(true);
	expect((await activePlayback(api, 41))?.activated_at).toBe(
		committed.application_timestamp,
	);
	expect((await activePlayback(api, 42))?.activated_at).toBe(
		committed.application_timestamp,
	);
	await bench.tick(2_500);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(1, 2);
	expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
		0.8,
		2,
	);
	await api.command("preload.release", {});
	expect(await activePlayback(api, 41)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 42)).toMatchObject({ enabled: true });
};

const preload004UiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	await prepare(
		api,
		bench,
		"preload-004-virtual-ui",
		[
			{
				number: 44,
				fixture: 3,
				levels: [1],
				name: "Virtual GO",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 45,
				fixture: 4,
				levels: [0.8],
				name: "Virtual TOGGLE",
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 46,
				fixture: 5,
				levels: [0.6],
				name: "Physical live",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 1: 44, 2: 45, 3: 46 },
	);
	await setCaptureMask(api, false, false, true, 2_500, 8_000);
	await desk.open(bench.baseUrl);
	const pane = await addVirtualPlaybackPane(page);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
	await api.executeCommandLine("GROUP 1 AT 35");
	await poolAction(api, 46, "button", {
		button: 1,
		pressed: true,
		surface: "physical",
	});
	await desk.recordStep(
		"QUEUE VIRTUAL CELLS",
		"Click the real GO and TOGGLE cells; their underlying playbacks must remain unchanged.",
	);
	await pane
		.getByRole("button", { name: /Virtual playback page 1 cell 1 Virtual GO/ })
		.click();
	await pane
		.getByRole("button", {
			name: /Virtual playback page 1 cell 2 Virtual TOGGLE/,
		})
		.click();
	await expect(
		page.getByTitle(/Pending Preload: .*GO 44.*TOGGLE 45/),
	).toBeVisible();
	expect(await activePlayback(api, 44)).toBeUndefined();
	expect(await activePlayback(api, 45)).toBeUndefined();
	expect(await activePlayback(api, 46)).toMatchObject({ enabled: true });
	await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
	await bench.tick(2_500);
	expect(await activePlayback(api, 44)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 45)).toMatchObject({ enabled: true });
	await longPressPreload(page);
	expect(await activePlayback(api, 44)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 45)).toMatchObject({ enabled: true });
};

export function registerVirtualPlaybackPreloadScenarios(): void {
	pairedScenario(preload004Scenario);
	test(
		"PRELOAD-004 @supplemental › API disabled-domain behavior and exact virtual transition timing",
		preload004ApiSupplement,
	);
	test(
		"PRELOAD-004 @supplemental-ui › virtual cells expose detailed pending feedback and release behavior",
		preload004UiSupplement,
	);
}
