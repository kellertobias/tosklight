import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";
import { object, objects, putObject } from "../support/catalog";
import {
	activePlayback,
	activeVirtualPane,
	addVirtualPlaybackPane,
	assignVirtualSource,
	chooseSelect,
	type PlaybackSpec,
	type Preload003State,
	pageObject,
	poolAction,
	prepare,
	writePage,
} from "./support";

const preload003Scenario: PairedScenario<Preload003State> = {
	id: "PRELOAD-003",
	title:
		"Virtual Playbacks use a persisted pane-native 2×2 grid and real GO/TOGGLE playbacks",
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
				buttons: ["go", "none", "none"],
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
			firstNumber: 101,
			secondNumber: 102,
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
								virtualPlaybackRows: 2,
								virtualPlaybackColumns: 2,
							},
						],
					},
				],
				activeDeskId: state.layoutDeskId,
			},
			existing?.revision ?? 0,
		);
		await writePage(api, 1, {
			"1": state.firstNumber,
			"2": state.secondNumber,
		});
		const second = await object<any>(
			api,
			"playback",
			String(state.secondNumber),
		);
		await putObject(
			api,
			"playback",
			String(state.secondNumber),
			{
				...second.body,
				buttons: ["toggle", "none", "none"],
			},
			second.revision,
		);
		await poolAction(api, state.firstNumber, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		await poolAction(api, state.secondNumber, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		expect(await activePlayback(api, state.firstNumber)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		expect(await activePlayback(api, state.secondNumber)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		const bootstrap = await api.request<any>(
			"GET",
			"/api/v1/bootstrap",
			undefined,
			false,
		);
		await api.request(
			"POST",
			`/api/v1/shows/${bootstrap.active_show.id}/open`,
			{ transition: "hold_current" },
		);
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await desk.recordStep(
			"CREATE VIRTUAL PLAYBACK PANE",
			"Add a normal configurable pane and set its grid to two rows by two columns.",
		);
		let pane = await addVirtualPlaybackPane(page);
		await pane.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Pane Settings" });
		await settings
			.getByRole("tab", { name: "Virtual Playbacks", exact: true })
			.click();
		await settings.getByLabel("Rows").fill("2");
		await settings.getByLabel("Columns").fill("2");
		await settings.getByRole("button", { name: "Close settings" }).click();
		await expect(pane.locator(".virtual-playback-cell")).toHaveCount(4);

		await assignVirtualSource(page, pane, "Virtual Source A", 1);
		pane = await activeVirtualPane(page);
		await assignVirtualSource(page, pane, "Virtual Source B", 2);
		pane = await activeVirtualPane(page);
		const pageState = await pageObject(api, 1);
		state.firstNumber = pageState.body.slots["1"];
		state.secondNumber = pageState.body.slots["2"];

		await page.getByRole("button", { name: "SET", exact: true }).click();
		await pane
			.getByRole("button", { name: /Virtual playback page 1 cell 2/ })
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
			.getByRole("button", { name: /Virtual playback page 1 cell 1/ })
			.click();
		await pane
			.getByRole("button", { name: /Virtual playback page 1 cell 2/ })
			.click();
		await expect
			.poll(async () => (await activePlayback(api, state.firstNumber))?.enabled)
			.toBe(true);
		await expect
			.poll(
				async () => (await activePlayback(api, state.secondNumber))?.enabled,
			)
			.toBe(true);

		await page.waitForTimeout(900);
		await page.reload();
		await expect(page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		pane = await activeVirtualPane(page);
		await expect(pane.locator(".virtual-playback-cell")).toHaveCount(4);
		await expect(
			pane.getByRole("button", { name: /Virtual playback page 1 cell 1/ }),
		).toContainText("GO");
		await expect(
			pane.getByRole("button", { name: /Virtual playback page 1 cell 2/ }),
		).toContainText("TOGGLE");
	},
	assert: async ({ api }, state) => {
		const pageState = await pageObject(api, 1);
		expect(pageState.body.slots).toMatchObject({
			"1": state.firstNumber,
			"2": state.secondNumber,
		});
		expect(
			await object<any>(api, "playback", String(state.firstNumber)),
		).toMatchObject({
			body: {
				button_count: 1,
				has_fader: false,
				buttons: ["go", "none", "none"],
			},
		});
		expect(
			await object<any>(api, "playback", String(state.secondNumber)),
		).toMatchObject({
			body: {
				button_count: 1,
				has_fader: false,
				buttons: ["toggle", "none", "none"],
			},
		});
		const layouts = await objects<any>(api, "user_layout");
		const pane = layouts
			.flatMap((layout) => layout.body.desks ?? [])
			.flatMap((desk: any) => desk.panes ?? [])
			.find((candidate: any) => candidate.kind === "virtual_playbacks");
		expect(pane).toEqual(
			expect.objectContaining({
				virtualPlaybackRows: 2,
				virtualPlaybackColumns: 2,
			}),
		);
	},
};

export function registerLayoutPersistenceScenarios(): void {
	pairedScenario(preload003Scenario);
}
