import type { Locator } from "@playwright/test";
import { expect, test } from "../bench/core/fixtures";
import { pairedScenario } from "../bench/core/pairedScenario";
import type {
	PlaybackConfigurationObservation,
	PreparedShow,
} from "../bench/playbacks/playback-configuration/models";
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

type Pbk001State = PreparedShow & {
	before: Awaited<ReturnType<typeof inertSnapshot>>;
	inspected?: PlaybackConfigurationObservation;
};

export function registerPbk001PairedScenario(): void {
	pairedScenario<Pbk001State>({
		id: "PBK-001",
		title:
			"Set inspection resolves one playback identity and Close is mutation-free",
		surfaces: ["api"],
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
			await playbackSnapshot(api);
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
	test("PBK-001 @supplemental › typed overview preserves page and slot state", async ({
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

	test("PBK-001 @ui › wider footprints keep the Playback's one- or two-button topology", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-001-footprint-buttons",
			"compact-rig",
		);
		const wider = {
			type: "wider" as const,
			right_buttons: ["go_minus", "go", "flash"] as [string, string, string],
			right_fader: "master",
		};
		await installPlaybacks(
			api,
			[
				definition(
					43,
					"One button wider",
					{ type: "cue_list", cue_list_id: prepared.cueListId },
					{
						buttons: ["go", "none", "none"],
						button_count: 1,
						has_fader: false,
						footprint: wider,
					},
				),
				definition(
					44,
					"Two buttons wider",
					{ type: "cue_list", cue_list_id: prepared.cueListId },
					{
						buttons: ["go_minus", "go", "none"],
						button_count: 2,
						has_fader: false,
						footprint: wider,
					},
				),
			],
			{ 1: 43, 4: 44 },
		);

		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		await expect(
			playbackCard(page, 1).locator("[data-playback-button-index]"),
		).toHaveCount(2);
		await expect(
			playbackCard(page, 4).locator("[data-playback-button-index]"),
		).toHaveCount(4);

		await playbackCard(page, 1).click({ button: "right" });
		const modal = await expectConfigurationModal(page, 1, 1);
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(
			modal.getByText("Right top button", { exact: true }),
		).toBeVisible();
		await expect(
			modal.getByText("Right middle button", { exact: true }),
		).toHaveCount(0);
		await expect(
			modal.getByText("Right bottom button", { exact: true }),
		).toHaveCount(0);
	});

	test("PBK-001 @ui › OFF makes the whole Playback target use its internal Off action", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-001-off-target",
			"compact-rig",
		);
		await installPlaybacks(
			api,
			[
				definition(45, "OFF target", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
				definition(46, "Running neighbor", {
					type: "cue_list",
					cue_list_id: prepared.cueListId,
				}),
			],
			{ 1: 45, 2: 46 },
		);
		await poolAction(api, 45, "go");
		await poolAction(api, 46, "go");

		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		await page.evaluate(() =>
			window.dispatchEvent(
				new CustomEvent("light:programmer-key", { detail: "off" }),
			),
		);
		await expect(page.getByLabel("Command line")).toHaveValue("OFF");
		await page.getByRole("button", { name: "Turn off OFF target" }).click();

		await expect
			.poll(async () => {
				const active = (await playbackSnapshot(api)).active;
				return {
					target: Boolean(
						active.find((item: any) => item.playback_number === 45),
					),
					neighbor: Boolean(
						active.find((item: any) => item.playback_number === 46),
					),
				};
			})
			.toEqual({ target: false, neighbor: true });
		await expect(
			page.getByRole("button", { name: "Turn off OFF target" }),
		).toHaveCount(0);
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
		).toHaveCount(0);
		await expect(
			pane.getByRole("button", { name: "Add Target", exact: true }),
		).toHaveCount(0);

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
		await pane
			.getByRole("button", {
				name: /Virtual playback 1001 page 1 cell 1 Virtual Sequence/,
			})
			.click({ button: "right" });
		let modal = await expectConfigurationModal(page, 1, 1);
		await modal
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
		expect(await inertSnapshot(api, 42)).toEqual(before);

		await armSet(page);
		await pane
			.getByRole("button", {
				name: /Virtual playback 1001 page 1 cell 1 Virtual Sequence/,
			})
			.click();
		modal = await expectConfigurationModal(page, 1, 1);
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
			.getByRole("button", {
				name: /Virtual playback 1002 page 1 cell 2 empty/,
			})
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
