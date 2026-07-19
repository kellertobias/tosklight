import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	armSet,
	choosePlaybackColor,
	clearSlot,
	definition,
	expectConfigurationModal,
	objects,
	openPlaybackMode,
	pageObject,
	playbackAt,
	playbackCard,
	playbackConfigurationObservation,
	prepareShow,
	saveSlot,
	selectTrigger,
	writePage,
} from "./helpers";
import type { PlaybackConfigurationObservation, PreparedShow } from "./models";
import { runPbk002AtomicConfigurationScenario } from "./pbk002Atomic";

type Pbk002State = PreparedShow & {
	assigned?: PlaybackConfigurationObservation;
};

export function registerPbk002PairedScenario(): void {
	pairedScenario<Pbk002State>({
		id: "PBK-002",
		title: "Cue List assignment, color, and None plus Apply clear are atomic",
		arrange: async ({ api, bench }, surface) => {
			const prepared = await prepareShow(
				api,
				bench,
				`pbk-002-paired-${surface}`,
				"default-stage",
			);
			await writePage(api, 1, {});
			return prepared;
		},
		api: async ({ api }, state) => {
			await saveSlot(
				api,
				1,
				1,
				definition(
					0,
					"Playback 1.1",
					{ type: "cue_list", cue_list_id: state.cueListId },
					{ color: "#8b5cf6" },
				),
			);
			state.assigned = await playbackConfigurationObservation(
				api,
				1,
				1,
				state.cueListId,
			);
			await clearSlot(api, 1, 1);
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			await openPlaybackMode(page);
			await armSet(page);
			await page
				.getByRole("button", {
					name: "Playback representation page 1 playback 1",
				})
				.click();
			let modal = await expectConfigurationModal(page, 1, 1);
			await modal
				.getByRole("radio", { name: "Configured Sequence", exact: true })
				.click();
			await choosePlaybackColor(page, modal, "#8b5cf6");
			await modal.getByRole("button", { name: "Apply", exact: true }).click();
			await expect(modal).toBeHidden();
			state.assigned = await playbackConfigurationObservation(
				api,
				1,
				1,
				state.cueListId,
			);
			await armSet(page);
			await page
				.getByRole("button", {
					name: "Playback representation page 1 playback 1",
				})
				.click();
			modal = await expectConfigurationModal(page, 1, 1);
			await modal.getByRole("radio", { name: "None", exact: true }).click();
			await expect(
				modal.getByText("Playback will be cleared", { exact: true }),
			).toBeVisible();
			await modal.getByRole("button", { name: "Apply", exact: true }).click();
			await expect(modal).toBeHidden();
		},
		assert: async ({ api }, state) => {
			expect(state.assigned).toMatchObject({
				page: 1,
				slot: 1,
				targetType: "cue_list",
				targetMatchesExpected: true,
				buttons: ["go_minus", "go", "flash"],
				buttonCount: 3,
				fader: "master",
				hasFader: true,
				color: "#8b5cf6",
			});
			expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
			expect(
				(await objects(api, "cue_list")).some(
					(item) => item.id === state.cueListId,
				),
			).toBe(true);
		},
	});
}

export function registerPbk002AtomicConfigurationScenario(): void {
	test(
		"PBK-002 @supplemental › every function, topology, migration, conflict, and reload path is atomic",
		runPbk002AtomicConfigurationScenario,
	);
}

export function registerPbk002LayoutUiScenario(): void {
	test("PBK-002 @supplemental-ui › grouped functions reset layout defaults and None plus Apply is explicit", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const prepared = await prepareShow(
			api,
			bench,
			"pbk-002-ui",
			"default-stage",
		);
		await writePage(api, 1, {});
		await desk.open(bench.baseUrl);
		await openPlaybackMode(page);
		await armSet(page);
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		let modal = await expectConfigurationModal(page, 1, 1);
		await expect(
			modal.getByRole("button", { name: "Apply", exact: true }),
		).toBeDisabled();
		await modal
			.getByRole("radio", { name: "Group Master", exact: true })
			.click();
		await expect(
			modal.getByRole("button", { name: "Apply", exact: true }),
		).toBeEnabled();
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(selectTrigger(modal, "Top button")).toContainText("Select");
		await expect(selectTrigger(modal, "Middle button")).toContainText(
			"Select dereferenced",
		);
		await expect(selectTrigger(modal, "Bottom button")).toContainText("Flash");
		await expect(selectTrigger(modal, "Fader")).toContainText(
			"Group intensity master",
		);
		await modal.getByRole("button", { name: "Function", exact: true }).click();
		await modal
			.getByRole("radio", { name: "Speed Master", exact: true })
			.click();
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(selectTrigger(modal, "Top button")).toContainText("Double");
		await expect(selectTrigger(modal, "Middle button")).toContainText("Half");
		await expect(selectTrigger(modal, "Bottom button")).toContainText("Learn");
		await expect(selectTrigger(modal, "Fader")).toContainText(
			"Learned-speed percentage",
		);
		await modal.getByRole("button", { name: "Function", exact: true }).click();
		await modal.getByRole("radio", { name: "Cue List", exact: true }).click();
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await expect(selectTrigger(modal, "Top button")).toContainText("GO −");
		await expect(selectTrigger(modal, "Middle button")).toContainText("GO +");
		await expect(selectTrigger(modal, "Bottom button")).toContainText("Flash");
		await modal.getByRole("button", { name: "Function", exact: true }).click();
		await choosePlaybackColor(page, modal, "#8b5cf6");
		await modal.getByRole("button", { name: "Behavior", exact: true }).click();
		await expect(
			modal.getByRole("radiogroup", {
				name: "When Flash or Swap is released",
				exact: true,
			}),
		).toBeVisible();
		await expect(
			modal.getByText(/leaves this Cue List active at zero intensity/),
		).toBeVisible();
		await expect(
			modal.getByRole("switch", {
				name: "Turn off when other playbacks take full control",
				exact: true,
			}),
		).toBeVisible();
		await modal
			.getByRole("radio", { name: "Intensity only", exact: true })
			.click();
		await modal
			.getByRole("switch", { name: "Protect from Swap", exact: true })
			.locator("..")
			.click();
		await modal.getByRole("button", { name: "Apply", exact: true }).click();
		await expect(modal).toBeHidden();

		let stored = await playbackAt(api, 1, 1);
		expect(stored.body).toMatchObject({
			target: { type: "cue_list", cue_list_id: prepared.cueListId },
			buttons: ["go_minus", "go", "flash"],
			color: "#8b5cf6",
			flash_release: "release_intensity_only",
			protect_from_swap: true,
		});
		await expect(playbackCard(page, 1)).toHaveCSS(
			"--playback-color",
			"#8b5cf6",
		);
		await page.reload();
		await openPlaybackMode(page);
		await expect(playbackCard(page, 1)).toHaveCSS(
			"--playback-color",
			"#8b5cf6",
		);

		await armSet(page);
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		modal = await expectConfigurationModal(page, 1, 1);
		await modal.getByRole("radio", { name: "None", exact: true }).click();
		await modal
			.getByRole("button", {
				name: "Close playback configuration",
				exact: true,
			})
			.click();
		stored = await playbackAt(api, 1, 1);
		expect(stored.body.color).toBe("#8b5cf6");
		await armSet(page);
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		modal = await expectConfigurationModal(page, 1, 1);
		await modal.getByRole("radio", { name: "None", exact: true }).click();
		await modal.getByRole("button", { name: "Apply", exact: true }).click();
		await expect(modal).toBeHidden();
		expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
		expect(
			(await objects(api, "cue_list")).some(
				(item) => item.id === prepared.cueListId,
			),
		).toBe(true);
	});
}
