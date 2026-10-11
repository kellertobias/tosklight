import { configurePaneGrid, paneCard } from "./uiSupport";
import { expect } from "../bench/core/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../bench/core/pairedScenario";
import { object, objects, putObject } from "../support/catalog";
import {
	activeVirtualPane,
	activeAssignedPlayback,
	addVirtualPlaybackPane,
	assignVirtualSource,
	chooseSelect,
	type PlaybackSpec,
	type Preload003State,
	pageObject,
	prepare,
	paneAction,
	copyPageAssignments,
} from "./support";

const preload003Scenario: PairedScenario<Preload003State> = {
	id: "PRELOAD-003",
	title:
		"Virtual Playbacks persist a full 20×15 Follow Main grid with dedicated GO/TOGGLE identities",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const specs: PlaybackSpec[] = [
			{
				position: 101,
				fixture: 3,
				levels: [0.2, 0.8],
				name: "Virtual Source A",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
			{
				position: 102,
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
			firstNumber: 401,
			secondNumber: 402,
			layoutDeskId: `preload-003-${surface}`,
		};
	},
	api: async ({ api }, state) => {
		const layoutId = api.session!.session_id;
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
								kind: "playbacks",
								title: "Virtual Playbacks",
								x: 1,
								y: 1,
								width: 12,
								height: 10,
 surface_id:"pane:preload-003",
							},
						],
					},
				],
				activeDeskId: state.layoutDeskId,
			},
			existing?.revision ?? 0,
		);
		await api.request("POST","/api/v2/playback-surfaces/pane%3Apreload-003/update",{
 request_id:crypto.randomUUID(),name:"Virtual Playbacks",layout:{rows:Array.from({length:20},(_,i)=>({first_position:401+i*15,count:15,presentation:"buttons",button_count:1}))},page_source:{type:"follow",surface_id:"main"}
 },true,undefined,{deskId:api.session!.desk.id});
		await copyPageAssignments(api, 1, { 401: 101, 402: 102 });
		await paneAction(api, {page: 1, position: state.firstNumber}, "go");
		await paneAction(api, {page: 1, position: state.secondNumber}, "toggle");
		expect(
			await activeAssignedPlayback(api, {page: 1, position: state.firstNumber}),
		).toMatchObject({
			enabled: true,
			current_cue_number: "1",
		});
		expect(
			await activeAssignedPlayback(api, {page: 1, position: state.secondNumber}),
		).toMatchObject({
			enabled: true,
			current_cue_number: "1",
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
		await configurePaneGrid(page,pane,20,15);
 await expect(pane.locator("[data-playback-slot]")).toHaveCount(300);
 // Operator-visible performance remains a separate measured acceptance gate.

		await assignVirtualSource(page, pane, "Virtual Source A", 1);
		pane = await activeVirtualPane(page);
		await assignVirtualSource(page, pane, "Virtual Source B", 2);
		pane = await activeVirtualPane(page);
		const pageState = await pageObject(api, 1);
		expect(pageState.body.assignments["401"]).toBeDefined();
		expect(pageState.body.assignments["402"]).toBeDefined();

		await page.getByRole("button", { name: "SET", exact: true }).click();
		await paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
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
		await paneCard(pane, {page:1,position:401}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
			.click();
		await paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
			.click();
		await expect
			.poll(
				async () =>
					(await activeAssignedPlayback(api, {page: 1, position: state.firstNumber}))?.enabled,
			)
			.toBe(true);
		await expect
			.poll(
				async () =>
					(await activeAssignedPlayback(api, {page: 1, position: state.secondNumber}))?.enabled,
			)
			.toBe(true);

		await page.waitForTimeout(900);
		await page.reload();
		await expect(page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		pane = await activeVirtualPane(page);
		await expect(pane.locator("[data-playback-slot]")).toHaveCount(300);
		await expect(
			paneCard(pane, {page:1,position:401}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/}),
		).toContainText("GO");
		await expect(
			paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/}),
		).toContainText("TOGGLE");
	},
	assert: async ({ api }, state) => {
		const pageState = await pageObject(api, 1);
		expect(pageState.body.assignments).toMatchObject({
			"401": {
				button_count: 1,
				has_fader: false,
				buttons: ["go", "none", "none"],
			},
			"402": {
				button_count: 1,
				has_fader: false,
				buttons: ["toggle", "none", "none"],
			},
		});
		expect(pageState.body.assignments[101]).toBeDefined();
		expect(pageState.body.assignments[102]).toBeDefined();
		const layouts = await objects<any>(api, "user_layout");
		const pane = layouts
			.flatMap((layout) => layout.body.desks ?? [])
			.flatMap((desk: any) => desk.panes ?? [])
			.find((candidate: any) => candidate.kind === "playbacks");
		expect(pane.surface_id).toEqual(expect.any(String));
 const snapshot=await api.request<any>("GET","/api/v2/playback-surfaces",undefined,true,undefined,{deskId:api.session!.desk.id});
 const configured=snapshot.playback_surfaces[pane.surface_id];
 expect(configured.layout.rows).toHaveLength(20);
 expect(configured.layout.rows.every((row:any)=>row.count===15&&row.button_count===1&&row.presentation==="buttons")).toBe(true);
 expect(configured.page_source).toEqual({type:"follow",surface_id:"main"});
	},
};

export function registerLayoutPersistenceScenarios(): void {
	pairedScenario(preload003Scenario);
}
