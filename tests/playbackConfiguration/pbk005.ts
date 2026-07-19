import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	activePlayback,
	createCueList,
	definition,
	hasSwapRuntime,
	hasTemporaryRuntime,
	installPlaybacks,
	intensityLevels,
	openPlaybackMode,
	playbackCard,
	playbackSnapshot,
	poolAction,
	prepareShow,
} from "./helpers";
import type { PreparedShow } from "./models";
import { runPbk005LifecycleScenario } from "./pbk005Lifecycle";

type Pbk005State = PreparedShow & {
	permanentBefore: Record<number, any>;
	levelsBefore: Record<number, number>;
	observations: {
		tempDuring?: boolean;
		tempAfter?: boolean;
		swapDuring?: boolean;
		swapAfter?: boolean;
		tempLevels?: Record<number, number>;
		swapLevels?: Record<number, number>;
	};
};

const pbk005PairedScenario: PairedScenario<Pbk005State> = {
	id: "PBK-005",
	title:
		"Temp and held Swap have explicit lifetimes and restore the underlying playback",
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepareShow(
			api,
			bench,
			`pbk-005-paired-${surface}`,
			"default-stage",
		);
		const configuration = await api.request<any>(
			"GET",
			"/api/v1/configuration",
		);
		await api.request("PUT", "/api/v1/configuration", {
			...configuration.configuration,
			sequence_master_fade_millis: 0,
		});
		const underlyingId = await createCueList(
			api,
			prepared.fixtures,
			"Underlying",
			[0.3],
			0,
			0,
			[1],
			false,
		);
		const temporaryId = await createCueList(
			api,
			prepared.fixtures,
			"Temporary",
			[0.8],
			0,
			0,
			[1],
			false,
		);
		const unprotectedId = await createCueList(
			api,
			prepared.fixtures,
			"Unprotected",
			[0.6],
			0,
			0,
			[2],
			false,
		);
		const protectedId = await createCueList(
			api,
			prepared.fixtures,
			"Protected",
			[0.4],
			0,
			0,
			[3],
			false,
		);
		await installPlaybacks(
			api,
			[
				definition(54, "Underlying", {
					type: "cue_list",
					cue_list_id: underlyingId,
				}),
				definition(
					55,
					"Temporary",
					{ type: "cue_list", cue_list_id: temporaryId },
					{ buttons: ["swap", "temp", "flash"] },
				),
				definition(
					56,
					"Unprotected",
					{ type: "cue_list", cue_list_id: unprotectedId },
					{ auto_off: false },
				),
				definition(
					57,
					"Protected",
					{ type: "cue_list", cue_list_id: protectedId },
					{ auto_off: false, protect_from_swap: true },
				),
			],
			{ 1: 54, 2: 55, 3: 56, 4: 57 },
		);
		await poolAction(api, 54, "on");
		await poolAction(api, 56, "on");
		await poolAction(api, 57, "on");
		await bench.tick(0);
		return {
			...prepared,
			permanentBefore: {
				54: await activePlayback(api, 54),
				56: await activePlayback(api, 56),
				57: await activePlayback(api, 57),
			},
			levelsBefore: await intensityLevels(api, prepared.fixtures, [1, 2, 3]),
			observations: {},
		};
	},
	api: async ({ api, bench }, state) => {
		await poolAction(api, 55, "temp");
		state.observations.tempDuring = Boolean(
			(await activePlayback(api, 55)).temporary_active,
		);
		await bench.tick(0);
		state.observations.tempLevels = await intensityLevels(
			api,
			state.fixtures,
			[1, 2, 3],
		);
		await poolAction(api, 55, "temp");
		state.observations.tempAfter = hasTemporaryRuntime(
			await playbackSnapshot(api),
			55,
		);
		await poolAction(api, 55, "swap", { pressed: true });
		state.observations.swapDuring = Boolean(
			(await activePlayback(api, 55)).swap_active,
		);
		await bench.tick(0);
		state.observations.swapLevels = await intensityLevels(
			api,
			state.fixtures,
			[1, 2, 3],
		);
		await poolAction(api, 55, "swap", { pressed: false });
		state.observations.swapAfter = hasSwapRuntime(
			await playbackSnapshot(api),
			55,
		);
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		const temp = playbackCard(page, 2).getByRole("button", {
			name: "TEMP",
			exact: true,
		});
		await temp.click();
		await expect
			.poll(async () => (await activePlayback(api, 55)).temporary_active)
			.toBe(true);
		state.observations.tempDuring = true;
		await bench.tick(0);
		state.observations.tempLevels = await intensityLevels(
			api,
			state.fixtures,
			[1, 2, 3],
		);
		await temp.click();
		await expect
			.poll(async () => hasTemporaryRuntime(await playbackSnapshot(api), 55))
			.toBe(false);
		state.observations.tempAfter = false;
		const swap = playbackCard(page, 2).getByRole("button", {
			name: "SWAP",
			exact: true,
		});
		await swap.hover();
		await page.mouse.down();
		try {
			await expect
				.poll(async () => hasSwapRuntime(await playbackSnapshot(api), 55))
				.toBe(true);
			state.observations.swapDuring = true;
			await bench.tick(0);
			state.observations.swapLevels = await intensityLevels(
				api,
				state.fixtures,
				[1, 2, 3],
			);
		} finally {
			await page.mouse.up();
		}
		await expect
			.poll(async () => hasSwapRuntime(await playbackSnapshot(api), 55))
			.toBe(false);
		state.observations.swapAfter = false;
	},
	assert: async ({ api }, state) => {
		expect(state.observations).toMatchObject({
			tempDuring: true,
			tempAfter: false,
			swapDuring: true,
			swapAfter: false,
		});
		expect(state.observations.tempLevels?.[1]).toBeCloseTo(0.8, 5);
		expect(state.observations.tempLevels?.[2]).toBeCloseTo(
			state.levelsBefore[2],
			5,
		);
		expect(state.observations.tempLevels?.[3]).toBeCloseTo(
			state.levelsBefore[3],
			5,
		);
		expect(state.observations.swapLevels?.[1]).toBeCloseTo(0.8, 5);
		expect(state.observations.swapLevels?.[2]).toBeCloseTo(0, 5);
		expect(state.observations.swapLevels?.[3]).toBeCloseTo(
			state.levelsBefore[3],
			5,
		);
		expect(await activePlayback(api, 54)).toEqual(state.permanentBefore[54]);
		expect(await activePlayback(api, 56)).toEqual(state.permanentBefore[56]);
		expect(await activePlayback(api, 57)).toEqual(state.permanentBefore[57]);
		const snapshot = await playbackSnapshot(api);
		expect(hasTemporaryRuntime(snapshot, 55)).toBe(false);
		expect(hasSwapRuntime(snapshot, 55)).toBe(false);
		const finalLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
		for (const number of [1, 2, 3])
			expect(finalLevels[number]).toBeCloseTo(state.levelsBefore[number], 5);
	},
};

export function registerPbk005PairedScenario(): void {
	pairedScenario(pbk005PairedScenario);
}

export function registerPbk005LifecycleScenario(): void {
	test(
		"PBK-005 @supplemental › Flash modes, auto-Off, Swap, and protection preserve permanent runtime",
		runPbk005LifecycleScenario,
	);
}

export function registerPbk005FeedbackScenario(): void {
	test("PBK-005 @supplemental-ui › held Swap and toggled Temp show detailed lifetime feedback", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-005-ui",
			"default-stage",
		);
		const aId = await createCueList(
			api,
			prepared.fixtures,
			"Underlying",
			[0.3],
			0,
			0,
			[1],
		);
		const bId = await createCueList(
			api,
			prepared.fixtures,
			"Temporary",
			[0.8],
			0,
			0,
			[1],
		);
		await installPlaybacks(
			api,
			[
				definition(54, "Underlying", { type: "cue_list", cue_list_id: aId }),
				definition(
					55,
					"Temporary",
					{ type: "cue_list", cue_list_id: bId },
					{ buttons: ["swap", "temp", "flash"] },
				),
			],
			{ 1: 54, 2: 55 },
		);
		await poolAction(api, 54, "on");
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		const swap = playbackCard(page, 2).getByRole("button", {
			name: "SWAP",
			exact: true,
		});
		await swap.hover();
		await page.mouse.down();
		await expect
			.poll(
				async () =>
					(await playbackSnapshot(api)).active.find(
						(item: any) => item.playback_number === 55,
					)?.swap_active ?? false,
			)
			.toBe(true);
		await expect(playbackCard(page, 2)).toHaveClass(/swap-active/);
		await page.mouse.up();
		await expect
			.poll(async () =>
				(await playbackSnapshot(api)).active.some(
					(item: any) => item.playback_number === 55 && item.swap_active,
				),
			)
			.toBe(false);
		expect((await activePlayback(api, 54)).enabled).toBe(true);

		const temp = playbackCard(page, 2).getByRole("button", {
			name: "TEMP",
			exact: true,
		});
		const [tempRequest] = await Promise.all([
			page.waitForRequest((request) =>
				request.url().endsWith("/api/v1/cuelists/55/button"),
			),
			temp.click(),
		]);
		expect(tempRequest.postDataJSON()).toMatchObject({
			button: 2,
			pressed: true,
			surface: "physical",
		});
		await expect
			.poll(async () => (await activePlayback(api, 55)).temporary_active)
			.toBe(true);
		await expect(temp).toHaveClass(/playback-button-active/);
		await temp.click();
		await expect
			.poll(async () =>
				(await playbackSnapshot(api)).active.some(
					(item: any) => item.playback_number === 55 && item.temporary_active,
				),
			)
			.toBe(false);
		expect((await activePlayback(api, 54)).enabled).toBe(true);
	});
}
