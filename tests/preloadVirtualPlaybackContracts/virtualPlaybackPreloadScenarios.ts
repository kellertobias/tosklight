import { paneCard } from "./uiSupport";
import {
	type BenchContractContext,
	type BenchUiContext,
	expect,
	test,
} from "../bench/core/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../bench/core/pairedScenario";
import {
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "../bench/programmer/programmerPreloadLifecycle";
import { clearProgrammerValues } from "../bench/programmer/programmerValues";
import { programmer } from "../support/catalog";
import {
	activeAssignedPlayback,
	addVirtualPlaybackPane,
	longPressPreload,
	openPlaybackMode,
	type PreloadVirtualPairState,
	playbackCard,
	assignedPlaybackAction,
	prepare,
	setCaptureScope,
	timestampMillis,
	paneAction,
	panePendingObservation,
	visualizationLevel,
	copyPageAssignments,
} from "./support";

const preload004Scenario: PairedScenario<PreloadVirtualPairState> = {
	id: "PRELOAD-004",
	title: "virtual GO and TOGGLE alone remain pending and share Programmer Fade",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepare(
			api,
			bench,
			`preload-004-paired-${surface}`,
			[
				{
					position: 44,
					fixture: 3,
					levels: [1],
					name: "Virtual GO",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					position: 45,
					fixture: 4,
					levels: [0.8],
					name: "Virtual TOGGLE",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					position: 46,
					fixture: 5,
					levels: [0.6],
					name: "Physical live",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 3: 46 },
		);
		await copyPageAssignments(api, 1, { 401: 44, 402: 45 });
		await setCaptureScope(api, false, {position_ranges: [{start:401,end:402}], pages:[1]}, 2_500, 8_000);
		return prepared;
	},
	api: async ({ api, bench }, state) => {
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		// Keep the disabled-domain programmer proof on a distinct fixture: programmer priority is
		// intentionally higher than these Cuelists and must not mask the playback fade under test.
		await api.executeCommandLine("FIXTURE 1 AT 35");
		await assignedPlaybackAction(api, { page: 1, position: 46 }, "button", {
			button: 1,
			pressed: true,

		});
		await paneAction(api, {page: 1, position: 401}, "go");
		await paneAction(api, {page: 1, position: 402}, "toggle");
		state.pendingActions = panePendingObservation(
			await programmer(api),
		);
		state.applicationTimestamp = (
			await goProgrammerPreload(api, {
				surface: "api",
				showId: state.showId,
			})
		).commit!.committedAt;
		await bench.tick(2_500);
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
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
		await paneCard(pane, {page:1,position:401}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
			.click();
		await paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
			.click();
		state.pendingActions = panePendingObservation(
			await programmer(api),
		);
		await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
		await expect
			.poll(async () => {
				state.applicationTimestamp =
					(await activeAssignedPlayback(api, {page: 1, position: 401}))?.activated_at;
				return state.applicationTimestamp ?? "";
			})
			.not.toBe("");
		await bench.tick(2_500);
		await longPressPreload(page);
	},
	assert: async ({ api }, state) => {
		expect(state.pendingActions).toEqual([
			[{page:1,position:401}, "go", {surface_id:expect.any(String),control_id:401}],
			[{page:1,position:402}, "toggle", {surface_id:expect.any(String),control_id:402}],
		]);
		expect(state.applicationTimestamp).toEqual(expect.any(String));
		expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
			enabled: true,
			current_cue_number: "1",
		});
		expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
			enabled: true,
			current_cue_number: "1",
		});
		expect(
			timestampMillis(
				(await activeAssignedPlayback(api, {page: 1, position: 401}))?.activated_at,
			),
		).toBe(timestampMillis(state.applicationTimestamp));
		expect(
			timestampMillis(
				(await activeAssignedPlayback(api, {page: 1, position: 402}))?.activated_at,
			),
		).toBe(timestampMillis(state.applicationTimestamp));
		expect(await activeAssignedPlayback(api, {page: 1, position: 46})).toMatchObject({
			enabled: true,
			current_cue_number: "1",
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
				position: 41,
				fixture: 3,
				levels: [1],
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				position: 42,
				fixture: 4,
				levels: [0.8],
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				position: 43,
				fixture: 5,
				levels: [0.6],
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 3: 43 },
	);
	await copyPageAssignments(api, 1, { 401: 41, 402: 42 });
	await setCaptureScope(api, false, {position_ranges: [{start:401,end:402}], pages:[1]}, 2_500, 8_000);
	await enterProgrammerPreload(api, {
		surface: "api",
		showId: prepared.showId,
	});
	await api.executeCommandLine("GROUP 1 AT 35");
	await assignedPlaybackAction(api, { page: 1, position: 43 }, "button", {
		button: 1,
		pressed: true,

	});
	await paneAction(api, {page: 1, position: 401}, "go");
	await paneAction(api, {page: 1, position: 402}, "toggle");
	const pending = await programmer(api);
	expect(pending.preload_group_pending).toEqual({});
	expect(
		panePendingObservation(pending),
	).toEqual([
		[{page:1,position:401}, "go", {surface_id:expect.any(String),control_id:401}],
		[{page:1,position:402}, "toggle", {surface_id:expect.any(String),control_id:402}],
	]);
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 41})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 42})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 43})).toMatchObject({ enabled: true });
	// The disabled programmer domain was proven live above; remove it before measuring the
	// independently captured virtual playback transition.
	await clearProgrammerValues(api, {
		surface: "api",
		showId: prepared.showId,
	});
	await bench.tick(100);
	const committed = (
		await goProgrammerPreload(api, {
			surface: "api",
			showId: prepared.showId,
		})
	).commit!;
	expect(committed.programmerFadeMillis).toBe(2_500);
	expect(committed.executed.map((entry) => entry.action)).toEqual([
		"go",
		"toggle",
	]);
	expect(
		timestampMillis(
			(await activeAssignedPlayback(api, {page: 1, position: 401}))?.activated_at,
		),
	).toBe(timestampMillis(committed.committedAt));
	expect(
		timestampMillis(
			(await activeAssignedPlayback(api, {page: 1, position: 402}))?.activated_at,
		),
	).toBe(timestampMillis(committed.committedAt));
	await bench.tick(2_500);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(1, 2);
	expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
		0.8,
		2,
	);
	await releaseProgrammerPreload(api, {
		surface: "api",
		showId: prepared.showId,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
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
				position: 44,
				fixture: 3,
				levels: [1],
				name: "Virtual GO",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				position: 45,
				fixture: 4,
				levels: [0.8],
				name: "Virtual TOGGLE",
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				position: 46,
				fixture: 5,
				levels: [0.6],
				name: "Physical live",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 3: 46 },
	);
	await copyPageAssignments(api, 1, { 401: 44, 402: 45 });
	await setCaptureScope(api, false, {position_ranges: [{start:401,end:402}], pages:[1]}, 2_500, 8_000);
	await desk.open(bench.baseUrl);
	const pane = await addVirtualPlaybackPane(page);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
	await api.executeCommandLine("GROUP 1 AT 35");
	await assignedPlaybackAction(api, { page: 1, position: 46 }, "button", {
		button: 1,
		pressed: true,

	});
	await desk.recordStep(
		"QUEUE VIRTUAL CELLS",
		"Click the real GO and TOGGLE cells; their underlying playbacks must remain unchanged.",
	);
	await paneCard(pane, {page:1,position:401}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
		.click();
	await paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
		.click();
	await expect(
		page.getByTitle(
			/Pending Preload: .*GO 1\.401.*TOGGLE 1\.402/,
		),
	).toBeVisible();
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 44})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 45})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 46})).toMatchObject({ enabled: true });
	await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
	await expect
		.poll(
			async () => (await programmer(api)).preload_playback_pending.length,
		)
		.toBe(0);
	await bench.tick(2_500);
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
	await longPressPreload(page);
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
};

export function registerVirtualPlaybackPreloadScenarios(): void {
	pairedScenario(preload004Scenario);
	test(
		"PRELOAD-004 @api › page-qualified Virtual preload capture preserves identity and exact transition timing",
		preload004ApiSupplement,
	);
	test(
		"PRELOAD-004 @ui › virtual cells expose detailed pending feedback and release behavior",
		preload004UiSupplement,
	);
}
