import { expect } from "../bench/core/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../bench/core/pairedScenario";
import { object, objects, putObject } from "../support/catalog";
import {
	activeVirtualPane,
	activeVirtualPlayback,
	addVirtualPlaybackPane,
	assignVirtualSource,
	chooseSelect,
	type PlaybackSpec,
	type Preload003State,
	pageObject,
	prepare,
	virtualAction,
	writeVirtualPage,
} from "./support";

const preload003Scenario: PairedScenario<Preload003State> = {
	id: "PRELOAD-003",
	title:
		"Virtual Playbacks persist a full 20×15 Follow Main grid with dedicated GO/TOGGLE identities",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const specs: PlaybackSpec[] = [
			{
				number: 101,
				fixture: 3,
				levels: [0.2, 0.8],
				name: "Virtual Source A",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				number: 102,
				fixture: 4,
				levels: [0.3, 0.9],
				name: "Virtual Source B",
				buttons: ["toggle", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		];
		const prepared = await prepare(
			api,
			bench,
			`preload-003-virtual-${surface}`,
			specs,
			{},
		);
		return {
			...prepared,
			firstNumber: 1001,
			secondNumber: 1002,
			layoutDeskId: `preload-003-${surface}`,
		};
	},
	api: async ({ api }, state) => {
		const layoutId = api.session!.user.id;
		const existing = (await objects<any>(api, "user_layout")).find(
			(entry) => entry.id === layoutId,
		);
		await putObject(
			api,
			"user_layout",
			layoutId,
			{
				desks: [
					{
						id: state.layoutDeskId,
						name: "Virtual Playback Desktop",
						panes: [
							{
								id: "virtual-playbacks-api",
								kind: "virtual_playbacks",
								title: "Virtual Playbacks",
								x: 1,
								y: 1,
								width: 12,
								height: 10,
								virtualPlaybackRows: 20,
								virtualPlaybackColumns: 15,
								virtualPlaybackPageMode: "follow_main",
								virtualPlaybackPinnedPage: 1,
							},
						],
					},
				],
				activeDeskId: state.layoutDeskId,
			},
			existing?.revision ?? 0,
		);
		await writeVirtualPage(api, 1, { 1001: 101, 1002: 102 });
		await virtualAction(api, 1, state.firstNumber, "go");
		await virtualAction(api, 1, state.secondNumber, "toggle");
		expect(
			await activeVirtualPlayback(api, 1, state.firstNumber),
		).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(
			await activeVirtualPlayback(api, 1, state.secondNumber),
		).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		const bootstrap = await api.request<any>(
			"GET",
			"/api/v2/bootstrap",
			undefined,
			false,
		);
		await api.openShow(bootstrap.active_show.id, {
			transition: "hold_current",
		});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await desk.recordStep(
			"CREATE VIRTUAL PLAYBACK PANE",
			"Add a normal configurable pane and set its full-page grid to 20 rows by 15 columns.",
		);
		let pane = await addVirtualPlaybackPane(page);
		await pane.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Pane Settings" });
		await settings
			.getByRole("tab", { name: "Virtual Playbacks", exact: true })
			.click();
		await settings.getByLabel("Rows").fill("20");
		await settings.getByLabel("Columns").fill("15");
		await settings
			.getByRole("radio", { name: "Follow Main", exact: true })
			.click();
		await settings.getByRole("button", { name: "Close settings" }).click();
		await expect(pane.locator(".virtual-playback-grid")).toHaveAttribute(
			"data-logical-cells",
			"300",
		);
		expect(await pane.locator(".virtual-playback-box").count()).toBeLessThan(
			200,
		);

		await assignVirtualSource(page, pane, "Virtual Source A", 1, "Cuelist 1");
		pane = await activeVirtualPane(page);
		await assignVirtualSource(page, pane, "Virtual Source B", 2, "Cuelist 2");
		pane = await activeVirtualPane(page);
		const pageState = await pageObject(api, 1);
		expect(pageState.body.virtual_playbacks["1001"]).toBeDefined();
		expect(pageState.body.virtual_playbacks["1002"]).toBeDefined();

		await page.getByRole("button", { name: "SET", exact: true }).click();
		await pane
			.getByRole("button", { name: /Virtual playback 1002 page 1 cell 2/ })
			.click();
		const modal = page.getByRole("dialog", { name: "Playback Configuration" });
		await expect(modal).toHaveAttribute(
			"data-topology",
			"1 button · faderless",
		);
		await modal.getByRole("button", { name: "Layout", exact: true }).click();
		await chooseSelect(page, modal, "Top button", "Toggle");
		await modal.getByRole("button", { name: "Apply", exact: true }).click();
		await expect(modal).toBeHidden();
		await pane
			.getByRole("button", { name: /Virtual playback 1001 page 1 cell 1/ })
			.click();
		await pane
			.getByRole("button", { name: /Virtual playback 1002 page 1 cell 2/ })
			.click();
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, state.firstNumber))?.enabled,
			)
			.toBe(true);
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, state.secondNumber))?.enabled,
			)
			.toBe(true);

		await page.waitForTimeout(900);
		await page.reload();
		await expect(page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		pane = await activeVirtualPane(page);
		await expect(pane.locator(".virtual-playback-grid")).toHaveAttribute(
			"data-logical-cells",
			"300",
		);
		await expect(
			pane.getByRole("button", { name: /Virtual playback 1001 page 1 cell 1/ }),
		).toContainText("GO");
		await expect(
			pane.getByRole("button", { name: /Virtual playback 1002 page 1 cell 2/ }),
		).toContainText("TOGGLE");
	},
	assert: async ({ api }, state) => {
		const pageState = await pageObject(api, 1);
		expect(pageState.body.virtual_playbacks).toMatchObject({
			"1001": {
				number: 1001,
				button_count: 1,
				has_fader: false,
				buttons: ["go", "none", "none"],
			},
			"1002": {
				number: 1002,
				button_count: 1,
				has_fader: false,
				buttons: ["toggle", "none", "none"],
			},
		});
		expect(await object<any>(api, "playback", "101")).toBeDefined();
		expect(await object<any>(api, "playback", "102")).toBeDefined();
		const layouts = await objects<any>(api, "user_layout");
		const pane = layouts
			.flatMap((layout) => layout.body.desks ?? [])
			.flatMap((desk: any) => desk.panes ?? [])
			.find((candidate: any) => candidate.kind === "virtual_playbacks");
		expect(pane).toEqual(
			expect.objectContaining({
				virtualPlaybackRows: 20,
				virtualPlaybackColumns: 15,
				virtualPlaybackPageMode: "follow_main",
			}),
		);
	},
};

export function registerLayoutPersistenceScenarios(): void {
	pairedScenario(preload003Scenario);
}
