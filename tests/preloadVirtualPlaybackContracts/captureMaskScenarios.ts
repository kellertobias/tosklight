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
} from "../../apps/control-ui/e2e/bench/programmerPreloadLifecycle";
import { loadCanonicalCopy, programmer } from "../support/catalog";
import {
	activePlayback,
	captureMask,
	configuration,
	expectPreloadMaskControls,
	openPreloadInputSettings,
	type PreloadMaskPairState,
	poolAction,
	prepare,
	setCaptureMask,
	setPreloadMaskThroughUi,
	timestampMillis,
} from "./support";

const preload005Scenario: PairedScenario<PreloadMaskPairState> = {
	id: "PRELOAD-005",
	title: "all eight capture-domain switch masks persist independently",
	arrange: async ({ api, bench }, surface) => {
		await loadCanonicalCopy(api, bench, `preload-005-paired-${surface}`);
		return { savedMasks: [] };
	},
	api: async ({ api }, state) => {
		for (let mask = 0; mask < 8; mask++) {
			await setCaptureMask(
				api,
				Boolean(mask & 1),
				Boolean(mask & 2),
				Boolean(mask & 4),
			);
			state.savedMasks.push(captureMask(await configuration(api)));
		}
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await openPreloadInputSettings(page);
		for (let mask = 0; mask < 8; mask++) {
			await setPreloadMaskThroughUi(api, page, mask);
			state.savedMasks.push(captureMask(await configuration(api)));
			await page.getByRole("button", { name: "Outputs", exact: true }).click();
			await page
				.getByRole("button", { name: "Programmer", exact: true })
				.click();
			await expectPreloadMaskControls(page, mask);
		}
	},
	assert: async ({ api }, state) => {
		const expected = Array.from(
			{ length: 8 },
			(_, mask): [boolean, boolean, boolean] => [
				Boolean(mask & 1),
				Boolean(mask & 2),
				Boolean(mask & 4),
			],
		);
		expect(state.savedMasks).toEqual(expected);
		expect(captureMask(await configuration(api))).toEqual([true, true, true]);
	},
};

const preload005ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const rows: Array<[boolean, boolean, boolean]> = Array.from(
		{ length: 8 },
		(_, mask) => [Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)],
	);
	for (const [programmerCapture, physicalCapture, virtualCapture] of rows) {
		const prepared = await prepare(
			api,
			bench,
			`preload-005-${Number(programmerCapture)}${Number(physicalCapture)}${Number(virtualCapture)}`,
			[
				{
					number: 51,
					fixture: 3,
					levels: [0.6],
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 52,
					fixture: 4,
					levels: [0.8],
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 1: 51, 2: 52 },
		);
		await setCaptureMask(
			api,
			programmerCapture,
			physicalCapture,
			virtualCapture,
			1_250,
			9_000,
		);
		const saved = await configuration(api);
		expect([
			saved.preload_programmer_changes,
			saved.preload_physical_playback_actions,
			saved.preload_virtual_playback_actions,
		]).toEqual([programmerCapture, physicalCapture, virtualCapture]);
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: prepared.showId,
		});
		await api.executeCommandLine("GROUP 1 AT 45");
		await poolAction(api, 51, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		await poolAction(api, 52, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		const pending = await programmer(api);
		expect(Boolean(pending.preload_group_pending["1"])).toBe(programmerCapture);
		expect(Boolean(pending.group_values["1"])).toBe(!programmerCapture);
		expect(
			pending.preload_playback_pending.map((entry: any) => entry.surface),
		).toEqual([
			...(physicalCapture ? ["physical"] : []),
			...(virtualCapture ? ["virtual"] : []),
		]);
		expect(Boolean(await activePlayback(api, 51))).toBe(!physicalCapture);
		expect(Boolean(await activePlayback(api, 52))).toBe(!virtualCapture);
		await bench.tick(100);
		const committed = (
			await goProgrammerPreload(api, {
				surface: "api",
				showId: prepared.showId,
			})
		).commit!;
		expect(committed.executed).toHaveLength(
			Number(physicalCapture) + Number(virtualCapture),
		);
		const after = await programmer(api);
		expect(Boolean(after.preload_group_active["1"])).toBe(programmerCapture);
		expect(after.preload_group_pending).toEqual({});
		expect(await activePlayback(api, 51)).toMatchObject({ enabled: true });
		expect(await activePlayback(api, 52)).toMatchObject({ enabled: true });
		if (physicalCapture)
			expect(
				timestampMillis((await activePlayback(api, 51))?.activated_at),
			).toBe(timestampMillis(committed.committedAt));
		if (virtualCapture)
			expect(
				timestampMillis((await activePlayback(api, 52))?.activated_at),
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
	await page.getByRole("button", { name: "Programmer", exact: true }).click();
	const labels = [
		"Preload programmer changes",
		"Preload physical playback actions",
		"Preload virtual playback actions",
	] as const;
	for (let mask = 0; mask < 8; mask++) {
		await desk.recordStep(
			`SAVE MASK ${mask + 1} / 8`,
			labels
				.map(
					(label, index) =>
						`${label.replace("Preload ", "")}: ${mask & (1 << index) ? "On" : "Off"}`,
				)
				.join(" · "),
		);
		for (let index = 0; index < labels.length; index++) {
			const desired = Boolean(mask & (1 << index));
			const control = page.getByRole("switch", { name: labels[index] });
			if ((await control.isChecked()) !== desired)
				await control.locator("..").locator(".ui-switch-track").click();
		}
		await page
			.getByRole("button", { name: "Save changes", exact: true })
			.click();
		await expect
			.poll(async () => {
				const current = await configuration(api);
				return [
					current.preload_programmer_changes,
					current.preload_physical_playback_actions,
					current.preload_virtual_playback_actions,
				];
			})
			.toEqual([Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
		await page.getByRole("button", { name: "Outputs", exact: true }).click();
		await page.getByRole("button", { name: "Programmer", exact: true }).click();
		for (let index = 0; index < labels.length; index++)
			await expect(page.getByLabel(labels[index])).toBeChecked({
				checked: Boolean(mask & (1 << index)),
			});
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
