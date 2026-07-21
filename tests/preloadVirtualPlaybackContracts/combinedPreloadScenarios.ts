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
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	programmer,
} from "../support/catalog";
import {
	activePlayback,
	addVirtualPlaybackPane,
	audit,
	firstGroupFixture,
	installOnCurrentShow,
	longPressPreload,
	openPlaybackMode,
	type PlaybackSpec,
	type PreloadCombinedPairState,
	playbackCard,
	playbacks,
	poolAction,
	preloadCombinedObservation,
	prepare,
	setCaptureMask,
	timestampMillis,
	visualizationLevel,
} from "./support";

const preload006Scenario: PairedScenario<PreloadCombinedPairState> = {
	id: "PRELOAD-006",
	title:
		"combined Preload commits atomically and releases only programmer data",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`preload-006-paired-${surface}`,
		);
		const fixtures = await fixtureIdsByNumber(api);
		const groupFixture = await firstGroupFixture(api, "1");
		const groupFixtureNumber = Number(
			Object.entries(fixtures).find(([, id]) => id === groupFixture)?.[0],
		);
		const prepared = await installOnCurrentShow(
			api,
			show.id,
			fixtures,
			[
				{
					number: 60,
					fixture: groupFixtureNumber,
					levels: [0.25],
					name: "Underlying source",
				},
				{
					number: 61,
					fixture: 3,
					levels: [0.6],
					name: "Physical combined",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 62,
					fixture: 4,
					levels: [0.8],
					name: "Virtual combined",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 1: 61, 2: 62 },
		);
		await setCaptureMask(api, true, true, true, 1_500, 8_000);
		await poolAction(api, 60, "go");
		await bench.tick(8_000);
		return { ...prepared, groupFixture };
	},
	api: async ({ api, bench }, state) => {
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.executeCommandLine("GROUP 1 AT 80");
		await poolAction(api, 61, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		await poolAction(api, 62, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		state.pending = preloadCombinedObservation(await programmer(api));
		state.applicationTimestamp = (
			await goProgrammerPreload(api, {
				surface: "api",
				showId: state.showId,
			})
		).commit!.committedAt;
		await bench.tick(1_500);
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		const pane = await addVirtualPlaybackPane(page);
		await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
		await desk.command("GROUP 1 AT 80", "G1 AT 80");
		await openPlaybackMode(page);
		await playbackCard(page, 1)
			.getByRole("button", { name: "GO +", exact: true })
			.click();
		await pane
			.getByRole("button", {
				name: /Virtual playback page 1 cell 2 Virtual combined/,
			})
			.click();
		state.pending = preloadCombinedObservation(await programmer(api));
		await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
		state.applicationTimestamp = (await programmer(api)).preload_group_active[
			"1"
		].intensity.changed_at;
		await bench.tick(1_500);
		await longPressPreload(page);
	},
	assert: async ({ api }, state) => {
		expect(state.pending).toEqual({
			groupIds: ["1"],
			playbackActions: [
				[61, "go", "physical"],
				[62, "toggle", "virtual"],
			],
		});
		expect(state.applicationTimestamp).toEqual(expect.any(String));
		expect(await activePlayback(api, 61)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(await activePlayback(api, 62)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(timestampMillis((await activePlayback(api, 61))?.activated_at)).toBe(
			timestampMillis(state.applicationTimestamp),
		);
		expect(timestampMillis((await activePlayback(api, 62))?.activated_at)).toBe(
			timestampMillis(state.applicationTimestamp),
		);
		expect((await programmer(api)).preload_group_active).toEqual({});
		expect(await visualizationLevel(api, state.groupFixture)).toBeCloseTo(
			0.25,
			2,
		);
	},
};

const preload006ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const show = await loadCanonicalCopy(
		api,
		bench,
		"preload-006-combined-release",
	);
	const groupFixture = await firstGroupFixture(api, "1");
	const fixtures = await fixtureIdsByNumber(api);
	const groupFixtureNumber = Number(
		Object.entries(fixtures).find(([, id]) => id === groupFixture)?.[0],
	);
	const specs: PlaybackSpec[] = [
		{
			number: 60,
			fixture: groupFixtureNumber,
			levels: [0.25],
			name: "Underlying source",
		},
		{
			number: 61,
			fixture: 3,
			levels: [0.6],
			name: "Physical result",
			buttons: ["go", "none", "none"],
			buttonCount: 1,
			hasFader: false,
		},
		{
			number: 62,
			fixture: 4,
			levels: [0.8],
			name: "Virtual result",
			buttons: ["toggle", "none", "none"],
			buttonCount: 1,
			hasFader: false,
		},
	];
	await installOnCurrentShow(api, show.id, fixtures, specs, { 1: 61, 2: 62 });
	await setCaptureMask(api, true, true, true, 1_500, 8_000);
	await poolAction(api, 60, "go");
	// This is an ordinary live GO, so its zero-time cue correctly uses the 8 s Cue Fade.
	await bench.tick(8_000);
	expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
	await enterProgrammerPreload(api, {
		surface: "api",
		showId: show.id,
	});
	await api.executeCommandLine("GROUP 1 AT 80");
	await poolAction(api, 61, "button", {
		button: 1,
		pressed: true,
		surface: "physical",
	});
	await poolAction(api, 62, "button", {
		button: 1,
		pressed: true,
		surface: "virtual",
	});
	const pending = await programmer(api);
	expect(pending.preload_group_pending["1"]).toBeDefined();
	expect(
		pending.preload_playback_pending.map((entry: any) => entry.surface),
	).toEqual(["physical", "virtual"]);
	expect(await activePlayback(api, 61)).toBeUndefined();
	expect(await activePlayback(api, 62)).toBeUndefined();

	await bench.tick(200);
	const committed = (
		await goProgrammerPreload(api, {
			surface: "api",
			showId: show.id,
		})
	).commit!;
	const committedProgrammer = await programmer(api);
	expect(
		timestampMillis(
			committedProgrammer.preload_group_active["1"].intensity.changed_at,
		),
	).toBe(timestampMillis(committed.committedAt));
	expect(timestampMillis((await activePlayback(api, 61))?.activated_at)).toBe(
		timestampMillis(committed.committedAt),
	);
	expect(timestampMillis((await activePlayback(api, 62))?.activated_at)).toBe(
		timestampMillis(committed.committedAt),
	);
	await bench.tick(0);
	expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
	expect(await visualizationLevel(api, fixtures[3])).toBeCloseTo(0, 5);
	expect(await visualizationLevel(api, fixtures[4])).toBeCloseTo(0, 5);
	await bench.tick(1_500);
	expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.8, 2);
	expect(await activePlayback(api, 61)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 62)).toMatchObject({ enabled: true });

	expect(
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: show.id,
		}),
	).toMatchObject({ status: "changed", active: false });
	expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
	expect(await activePlayback(api, 61)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 62)).toMatchObject({ enabled: true });
	const eventsBefore = await audit(api);
	const frameBefore = await bench.tick(0);
	expect(
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: show.id,
		}),
	).toMatchObject({ status: "no_change", active: false });
	expect(
		await audit(
			api,
			Math.max(0, ...eventsBefore.map((event) => event.revision)),
		),
	).toEqual([]);
	expect(await bench.tick(0)).toEqual(frameBefore);
};

const preload006UiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	await prepare(
		api,
		bench,
		"preload-006-ui",
		[
			{
				number: 63,
				fixture: 3,
				levels: [0.6],
				name: "Physical combined",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 64,
				fixture: 4,
				levels: [0.8],
				name: "Virtual combined",
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 1: 63, 2: 64 },
	);
	await setCaptureMask(api, true, true, true, 1_500, 8_000);
	await desk.open(bench.baseUrl);
	const pane = await addVirtualPlaybackPane(page);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
	await desk.command("GROUP 1 AT 80", "G1 AT 80");
	await openPlaybackMode(page);
	await playbackCard(page, 1)
		.getByRole("button", { name: "GO +", exact: true })
		.click();
	await pane
		.getByRole("button", {
			name: /Virtual playback page 1 cell 2 Virtual combined/,
		})
		.click();
	await expect(
		page.getByTitle(/Pending Preload: PROG 1.*GO 63.*TOGGLE 64/),
	).toBeVisible();
	await desk.recordStep(
		"ATOMIC PRELOAD GO",
		"Publish the temporary programmer and both real playbacks at one application timestamp.",
	);
	await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
	await bench.tick(1_500);
	const state = await playbacks(api);
	const physical = state.active.find(
		(entry: any) => entry.playback_number === 63,
	);
	const virtual = state.active.find(
		(entry: any) => entry.playback_number === 64,
	);
	expect(physical.activated_at).toBe(virtual.activated_at);
	await desk.recordStep(
		"LONG-PRESS RELEASE",
		"Remove only the temporary programmer; the physical and virtual playback results remain.",
	);
	await longPressPreload(page);
	expect((await programmer(api)).preload_group_active).toEqual({});
	expect(await activePlayback(api, 63)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 64)).toMatchObject({ enabled: true });
};

export function registerCombinedPreloadScenarios(): void {
	pairedScenario(preload006Scenario);
	test(
		"PRELOAD-006 @supplemental › API timestamp boundaries, source ownership, and event idempotency",
		preload006ApiSupplement,
	);
	test(
		"PRELOAD-006 @supplemental-ui › combined controls expose pending state and asymmetric long-press release",
		preload006UiSupplement,
	);
}
