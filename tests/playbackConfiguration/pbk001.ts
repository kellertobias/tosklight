import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import type { Locator } from "../../apps/control-ui/node_modules/@playwright/test/index.js";
import {
	addVirtualPlaybackPane,
	armSet,
	chooseSelect,
	definition,
	expectConfigurationModal,
	inertSnapshot,
	installPlaybacks,
	openPlaybackMode,
	pageObject,
	playbackCard,
	playbackConfigurationObservation,
	playbackSnapshot,
	poolAction,
	prepareShow,
	selectTrigger,
} from "./helpers";
import type { PlaybackConfigurationObservation, PreparedShow } from "./models";

type Pbk001State = PreparedShow & {
	before: Awaited<ReturnType<typeof inertSnapshot>>;
	inspected?: PlaybackConfigurationObservation;
};

export function registerPbk001PairedScenario(): void {
	pairedScenario<Pbk001State>({
		id: "PBK-001",
		title:
			"Set inspection resolves one playback identity and Close is mutation-free",
		arrange: async ({ api, bench }, surface) => {
			const prepared = await prepareShow(
				api,
				bench,
				`pbk-001-paired-${surface}`,
				"compact-rig",
			);
			await installPlaybacks(
				api,
				[
					definition(40, "Configured Sequence", {
						type: "cue_list",
						cue_list_id: prepared.cueListId,
					}),
				],
				{ 1: 40 },
			);
			await poolAction(api, 40, "go");
			await poolAction(api, 40, "master", { value: 0.6 });
			return { ...prepared, before: await inertSnapshot(api, 40) };
		},
		api: async ({ api }, state) => {
			await api.request("GET", "/api/v1/playback-pool/40");
			state.inspected = await playbackConfigurationObservation(
				api,
				1,
				1,
				state.cueListId,
			);
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			await openPlaybackMode(page);
			state.before = await inertSnapshot(api, 40);
			await armSet(page);
			await page
				.getByRole("button", {
					name: "Playback representation page 1 playback 1",
				})
				.click();
			const modal = await expectConfigurationModal(page, 1, 1);
			await expect(
				modal.getByRole("button", { name: "Function", exact: true }),
			).toBeVisible();
			await expect(
				modal.getByRole("button", { name: "Behavior", exact: true }),
			).toBeVisible();
			await expect(
				modal.getByRole("button", { name: "Layout", exact: true }),
			).toBeVisible();
			state.inspected = await playbackConfigurationObservation(
				api,
				1,
				1,
				state.cueListId,
			);
			await modal
				.getByRole("button", {
					name: "Close playback configuration",
					exact: true,
				})
				.click();
			await expect(modal).toBeHidden();
		},
		assert: async ({ api }, state) => {
			expect(state.inspected).toEqual({
				page: 1,
				slot: 1,
				number: 40,
				targetType: "cue_list",
				targetMatchesExpected: true,
				buttons: ["go_minus", "go", "flash"],
				buttonCount: 3,
				fader: "master",
				hasFader: true,
				color: "#20c997",
			});
			expect(await inertSnapshot(api, 40)).toEqual(state.before);
		},
	});
}

export function registerPbk001ReadApiScenario(): void {
	test("PBK-001 @supplemental › direct and legacy read APIs preserve page/slot state", async ({
		api,
		bench,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-001-api",
			"compact-rig",
		);
		await installPlaybacks(
			api,
			[
				definition(40, "API Identity", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
			],
			{ 1: 40 },
		);
		await poolAction(api, 40, "go");
		await poolAction(api, 40, "master", { value: 0.6 });
		const before = await inertSnapshot(api, 40);
		const snapshot = await playbackSnapshot(api);
		expect(
			snapshot.pages.find((candidate: any) => candidate.number === 1)?.slots[
				"1"
			],
		).toBe(40);
		expect(
			snapshot.pool.find((candidate: any) => candidate.number === 40),
		).toEqual(before.object.body);
		const direct = await api.request<any>("GET", "/api/v1/playback-pool/40");
		const legacyAlias = await api.request<any>("GET", "/api/v1/cuelists/40");
		expect(direct.playback).toEqual(before.object.body);
		expect(legacyAlias.playback).toEqual(before.object.body);
		expect(await inertSnapshot(api, 40)).toEqual(before);
	});
}

export function registerPbk001PhysicalControlsScenario(): void {
	test("PBK-001 @supplemental-ui › SET intercepts every physical control without operating it", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-001-physical",
			"compact-rig",
		);
		const playback = definition(41, "Configured Sequence", {
			type: "cue_list",
			cue_list_id: prepared.cueListId,
		});
		await installPlaybacks(api, [playback], { 1: 41 });
		await poolAction(api, 41, "go");
		await poolAction(api, 41, "master", { value: 0.6 });

		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		const before = await inertSnapshot(api, 41);
		const surfaces: Array<[string, () => Locator]> = [
			[
				"top button",
				() =>
					playbackCard(page, 1).getByRole("button", {
						name: "GO −",
						exact: true,
					}),
			],
			[
				"middle button",
				() =>
					playbackCard(page, 1).getByRole("button", {
						name: "GO +",
						exact: true,
					}),
			],
			[
				"bottom button",
				() =>
					playbackCard(page, 1).getByRole("button", {
						name: "FLASH",
						exact: true,
					}),
			],
			[
				"fader track and handle",
				() => playbackCard(page, 1).getByRole("slider", { name: "Master" }),
			],
			[
				"software representation",
				() =>
					page.getByRole("button", {
						name: "Playback representation page 1 playback 1",
					}),
			],
		];

		for (const [surface, target] of surfaces) {
			await test.step(`SET then ${surface}`, async () => {
				await armSet(page);
				await target().click();
				const modal = await expectConfigurationModal(page, 1, 1);
				await expect(
					modal.getByRole("button", { name: "Function", exact: true }),
				).toBeVisible();
				await expect(
					modal.getByRole("button", { name: "Behavior", exact: true }),
				).toBeVisible();
				await expect(
					modal.getByRole("button", { name: "Layout", exact: true }),
				).toBeVisible();
				await modal
					.getByRole("button", {
						name: "Close playback configuration",
						exact: true,
					})
					.click();
				await expect(modal).toBeHidden();
				expect(await inertSnapshot(api, 41)).toEqual(before);
			});
		}

		await armSet(page);
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 2",
			})
			.click();
		const empty = await expectConfigurationModal(page, 1, 2);
		await expect(empty.getByRole("radio", { name: "None" })).toBeVisible();
		await empty
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
		expect((await pageObject(api, 1)).body.slots["2"]).toBeUndefined();
		expect(await inertSnapshot(api, 41)).toEqual(before);
	});
}

export function registerPbk001VirtualCellsScenario(): void {
	test("PBK-001 @supplemental-ui › Virtual cells share the modal with one-button topology and presentation", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-001-virtual",
			"compact-rig",
		);
		await installPlaybacks(
			api,
			[
				definition(
					42,
					"Virtual Sequence",
					{ type: "cue_list", cue_list_id: prepared.cueListId },
					{
						buttons: ["toggle", "none", "none"],
						button_count: 1,
						has_fader: false,
						presentation_icon: "▶",
					},
				),
			],
			{ 1: 42 },
		);
		await desk.open(bench.baseUrl);
		const pane = await addVirtualPlaybackPane(page);
		await expect(
			pane.getByRole("button", { name: "Set Source", exact: true }),
		).toBeVisible();
		await expect(
			pane.getByRole("button", { name: "Add Target", exact: true }),
		).toBeVisible();

		await pane.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Pane Settings" });
		await settings
			.getByRole("tab", { name: "Virtual Playbacks", exact: true })
			.click();
		await expect(settings.getByLabel("Rows")).toBeVisible();
		await expect(settings.getByLabel("Columns")).toBeVisible();
		await expect(
			settings.getByText(/Cuelist assignment|Action assignment/i),
		).toHaveCount(0);
		await settings.getByRole("button", { name: "Close settings" }).click();

		const before = await inertSnapshot(api, 42);
		await armSet(page);
		await pane
			.getByRole("button", {
				name: /Virtual playback page 1 cell 1 Virtual Sequence/,
			})
			.click();
		let modal = await expectConfigurationModal(page, 1, 1);
		await expect(modal).toHaveAttribute(
			"data-topology",
			"1 button · faderless",
		);
		await expect(selectTrigger(modal, "Presentation")).toBeVisible();
		await chooseSelect(page, modal, "Presentation", "Image background");
		await expect(modal.getByLabel("Image background")).toBeVisible();
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(selectTrigger(modal, "Top button")).toBeVisible();
		await expect(selectTrigger(modal, "Middle button")).toHaveCount(0);
		await expect(
			modal.getByText("No fader on this playback.", { exact: true }),
		).toBeVisible();
		await modal
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
		expect(await inertSnapshot(api, 42)).toEqual(before);

		await armSet(page);
		await pane
			.getByRole("button", { name: /Virtual playback page 1 cell 2 empty/ })
			.click();
		modal = await expectConfigurationModal(page, 1, 2);
		await expect(modal).toHaveAttribute(
			"data-topology",
			"1 button · faderless",
		);
		await expect(selectTrigger(modal, "Presentation")).toBeVisible();
		await modal
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
		expect((await pageObject(api, 1)).body.slots["2"]).toBeUndefined();
	});
}
