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
} from "../bench/programmer/programmerPreloadLifecycle";
import { captureMasks, scopeContains } from "./scopeCases";
import { loadCanonicalCopy, programmer } from "../support/catalog";
import {
	activeAssignedPlayback,
	captureMask,
	configuration,
	expectPreloadMaskControls,
	openPreloadInputSettings,
	type PreloadMaskPairState,
	assignedPlaybackAction,
	prepare,
	setCaptureScope,
	setPreloadMaskThroughUi,
	timestampMillis,
} from "./support";

const preload005Scenario: PairedScenario<PreloadMaskPairState> = {
	id: "PRELOAD-005",
	title: "programmer capture and four Playback eligibility scopes persist independently",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		await loadCanonicalCopy(api, bench, `preload-005-paired-${surface}`);
		return { savedMasks: [] };
	},
	api: async ({ api }, state) => {
		for (const mask of captureMasks()) {
			await setCaptureScope(api,mask.programmer,mask.playbackScope);
			state.savedMasks.push(await captureMask(api));
		}
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await openPreloadInputSettings(page);
		for (const mask of captureMasks()) {
			await setPreloadMaskThroughUi(api, page, mask);
			state.savedMasks.push(await captureMask(api));
			await page.getByRole("button", { name: "Outputs", exact: true }).click();
			await page
				.getByRole("button", { name: "Others", exact: true })
				.click();
			await expectPreloadMaskControls(page, mask);
		}
	},
	assert: async ({ api }, state) => {
		expect(state.savedMasks).toEqual(captureMasks());
		expect(await captureMask(api)).toEqual(captureMasks().at(-1));
	},
};

const preload005ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	for (const mask of captureMasks()) {
 const { programmer:programmerCapture,playbackScope }=mask;
 const firstCapture=scopeContains(playbackScope,{page:1,position:51});
 const secondCapture=scopeContains(playbackScope,{page:1,position:52});
		const prepared = await prepare(
			api,
			bench,
			`preload-005-${Number(programmerCapture)}${Number(firstCapture)}${Number(secondCapture)}`,
			[
				{
					position: 51,
					fixture: 3,
					levels: [0.6],
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					position: 52,
					fixture: 4,
					levels: [0.8],
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 1: 51, 2: 52 },
		);
		await setCaptureScope(api,programmerCapture,playbackScope,1250,9000);
		expect(await captureMask(api)).toEqual(mask);
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: prepared.showId,
		});
		await api.executeCommandLine("GROUP 1 AT 45");
		await assignedPlaybackAction(api, { page: 1, position: 51 }, "button", {
			button: 1,
			pressed: true,

		});
		await assignedPlaybackAction(api, { page: 1, position: 52 }, "button", {
			button: 1,
			pressed: true,

		});
		const pending = await programmer(api);
		expect(Boolean(pending.preload_group_pending["1"])).toBe(programmerCapture);
		expect(Boolean(pending.group_values["1"])).toBe(!programmerCapture);
		expect(pending.preload_playback_pending.map((entry:any)=>entry.address)).toEqual([
 ...(firstCapture?[{page:1,position:51}]:[]),...(secondCapture?[{page:1,position:52}]:[])
]);
		expect(Boolean(await activeAssignedPlayback(api, {page: 1, position: 51}))).toBe(!firstCapture);
		expect(Boolean(await activeAssignedPlayback(api, {page: 1, position: 52}))).toBe(!secondCapture);
		await bench.tick(100);
		const committed = (
			await goProgrammerPreload(api, {
				surface: "api",
				showId: prepared.showId,
			})
		).commit!;
		expect(committed.executed).toHaveLength(
			Number(firstCapture) + Number(secondCapture),
		);
		const after = await programmer(api);
		expect(Boolean(after.preload_group_active["1"])).toBe(programmerCapture);
		expect(after.preload_group_pending).toEqual({});
		expect(await activeAssignedPlayback(api, {page: 1, position: 51})).toMatchObject({ enabled: true });
		expect(await activeAssignedPlayback(api, {page: 1, position: 52})).toMatchObject({ enabled: true });
		if (firstCapture)
			expect(
				timestampMillis((await activeAssignedPlayback(api, {page: 1, position: 51}))?.activated_at),
			).toBe(timestampMillis(committed.committedAt));
		if (secondCapture)
			expect(
				timestampMillis((await activeAssignedPlayback(api, {page: 1, position: 52}))?.activated_at),
			).toBe(timestampMillis(committed.committedAt));
	}
};

const preload005UiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	await loadCanonicalCopy(api, bench, "preload-005-settings");
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page.getByRole("button", { name: "Others", exact: true }).click();
	for (const [index,mask] of captureMasks().entries()) {
 await desk.recordStep(`SAVE ELIGIBILITY ${index+1} / 8`,JSON.stringify(mask));
 await setPreloadMaskThroughUi(api,page,mask);
 await page.getByRole("button",{name:"Outputs",exact:true}).click();
 await page.getByRole("button",{name:"Others",exact:true}).click();
 await expectPreloadMaskControls(page,mask);
}
};

export function registerCaptureMaskScenarios(): void {
	pairedScenario(preload005Scenario);
	test(
		"PRELOAD-005 @supplemental › every mask keeps disabled domains live and enabled domains blind",
		preload005ApiSupplement,
	);
	test(
		"PRELOAD-005 @supplemental-ui › Settings visibly reloads every independent switch mask",
		preload005UiSupplement,
	);
}
