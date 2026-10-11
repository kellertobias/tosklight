import { assignPlayback } from "./cueSemanticContracts/playbackFixtures";
import type { PlaybackAddress } from "../apps/light-desktop/src/features/playbackTopology/contracts";
import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { PairedScenario } from "./bench/core/pairedScenario";
import { setProgrammerFixtureValue } from "./bench/programmer/programmerValues";
import type {
	Locator,
	Page,
} from "@playwright/test";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	objects,
	programmer,
	putObject,
} from "./support/catalog";

type Prepared = { firstCuelist: string; secondCuelist: string; showId: string };

const selectContract: Omit<PairedScenario<Prepared>,"ui"> = {
	id: "PLAYBACK-SELECT-001",
	title:
		"A hardware-connected playback card selects its concrete Cuelist playback",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) =>
		prepare(api, bench, `playback-select-001-${surface}`),
	api: async ({ api }) => {
		await selectPlayback(api,{page:1,position:1});
	},
	assert: async ({ api }) => {
		expect((await playbackState(api)).selected_playback).toEqual({page:1,position:1});
	},
};
test(
	"PLAYBACK-SELECT-001 @api › exact Playback selection resolves its assigned target",
	async ({ api, bench, show, request, desk, page }) => {
		const context = { api, bench, show, request, desk, page };
		const state = await selectContract.arrange(context, "api");
		await selectContract.api(context, state);
		await selectContract.assert(context, state, "api");
	},
);

test("PLAYBACK-SELECT-001 @supplemental-ui › controls, Record, Group selection, and explicit pages retain separate ownership", async ({ api, bench, desk, page }) => {
  const prepared = await prepare(api, bench, "playback-select-001-boundaries");
  const fixtures = await fixtureIdsByNumber(api);
  await replaceProgrammingSelection(api, {
    surface: "api",
    showId: prepared.showId,
    fixtures: [fixtures[1]],
  });
  await setProgrammerFixtureValue(api, {
    surface: "api",
    showId: prepared.showId,
    fixtureId: fixtures[1],
    attribute: "intensity",
    value: { kind: "normalized", value: 0.73 },
    timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
  });

  await desk.open(api.baseUrl);
  await openPlaybackMode(page);
  const hardware = await connectHardware(api, bench);
  try {
    const first = playbackCard(page, 1);
    await first.locator("header b").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toEqual({page:1,position:1});

    await first.getByRole("button", { name: "GO +", exact: true }).click();
    await first.getByRole("slider").evaluate((element: HTMLInputElement) => {
      element.value = "37";
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect((await playbackState(api)).selected_playback).toEqual({page:1,position:1});

    const beforeFirst = (await object<any>(api, "cue_list", prepared.firstCuelist)).body.cues.length;
    const beforeSecond = (await object<any>(api, "cue_list", prepared.secondCuelist)).body.cues.length;
    await page.getByRole("button", { name: "REC", exact: true }).click();
    await first.getByRole("button", { name: "GO +", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", prepared.firstCuelist)).body.cues.length).toBe(beforeFirst + 1);
    expect((await object<any>(api, "cue_list", prepared.secondCuelist)).body.cues).toHaveLength(beforeSecond);
    expect((await playbackState(api)).selected_playback).toEqual({page:1,position:1});

    await returnToPlaybackDesk(page);
    await selectHardwarePage(page, 1, 2, "Page 2");
    await playbackCard(page, 1).locator("header b").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toEqual({page:2,position:1});
    await expect(page.locator(".ui-window-header")).toContainText("Cuelist View · Cuelist 42 · Rear Cuelist");
    await returnToPlaybackDesk(page);
    await selectHardwarePage(page, 2, 1, "Main");
    expect((await playbackState(api)).selected_playback).toEqual({page:2,position:1});

    await playbackCard(page, 2).locator(".hardware-cue-list").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toEqual({page:1,position:2});
    await expect.poll(async () => (await programmer(api)).selected).toContain(fixtures[1]);
    await expect(page.locator(".playback-fader-bank")).toBeVisible();

    await selectPlayback(api,{page:1,position:1});
    await hardware.send(`/light/desk/playback/1/2/button/1`, [true]);
    await expect.poll(async () => (await playbackState(api)).selected_playback).toEqual({page:1,position:2});
  } finally {
    await disconnectHardware(hardware);
  }
});

async function prepare(
	api: ApiDriver,
	bench: any,
	name: string,
): Promise<Prepared> {
	const show = await loadCanonicalCopy(api, bench, name, "default-stage");
	const fixtures = await fixtureIdsByNumber(api);
	const firstCuelist = crypto.randomUUID();
	const secondCuelist = crypto.randomUUID();
	await putObject(
		api,
		"cue_list",
		firstCuelist,
		cueList(firstCuelist, "Front Cuelist",41),
	);
	await putObject(
		api,
		"cue_list",
		secondCuelist,
		cueList(secondCuelist, "Rear Cuelist",42),
	);
	const existingGroup = (await objects<any>(api, "group")).find(
		(candidate) => candidate.id === "1",
	);
	await putObject(
		api,
		"group",
		"1",
		{
			id: "1",
			name: "Hardware Group",
			fixtures: [fixtures[1]],
			derived_from: null,
			frozen_from: null,
			programming: {},
		},
		existingGroup?.revision ?? 0,
	);
	await assignPlayback(api,{page:1,position:1},playback("Front Cuelist",{type:"cue_list",cue_list_id:firstCuelist}));
 await assignPlayback(api,{page:2,position:1},playback("Rear Cuelist",{type:"cue_list",cue_list_id:secondCuelist}));
 await assignPlayback(api,{page:1,position:2},playback("Hardware Group",{type:"group",group_id:"1"},["select","flash","select_dereferenced"]));
	return { firstCuelist, secondCuelist, showId: show.id };
}

function cueList(id: string, name: string,poolNumber:number) {
	return {
		id,
		name,
		pool_number:poolNumber,
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues: [
			{
				id: crypto.randomUUID(),
				number: 1,
				name: "Opening",
				changes: [],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
			},
		],
	};
}

function playback(
	name: string,
	target: any,
	buttons: [string, string, string] = ["go_minus", "go", "flash"],
) {
	return {
		name,
		target,
		buttons,
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

async function selectPlayback(api: ApiDriver, address:PlaybackAddress) {
	await api.playbackAction(address, "select", {});
}

async function playbackState(api: ApiDriver) {
	return api.request<any>("GET", "/api/v2/playback-overview");
}

async function openPlaybackMode(page: Page) {
	if (await page.locator(".playback-fader-bank").isVisible()) return;
	await page.locator(".mode-toggle").click();
	await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

async function returnToPlaybackDesk(page: Page) {
	await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
	await page.locator(".dock-list .dock-entry").first().click();
	await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

async function selectHardwarePage(
	page: Page,
	current: number,
	target: number,
	name: string,
) {
	await page
		.getByRole("button", { name: `Page ${current}`, exact: true })
		.click();
	const dialog = page.getByRole("dialog", { name: "Playback pages" });
	await dialog
		.getByRole("button", { name: `${target} ${name}`, exact: true })
		.click();
	await expect(
		page.getByRole("button", { name: `Page ${target}`, exact: true }),
	).toBeVisible();
}

function playbackCard(page: Page, slot: number): Locator {
	return page.locator(
		`.playback-fader-bank article[data-playback-slot="${slot}"]`,
	);
}

async function connectHardware(api: ApiDriver, bench: any) {
	const hardware = await bench.osc();
	await hardware.subscribe(
		`playback-select-${crypto.randomUUID()}`,
		"desk",
	);
	await expect
		.poll(
			async () =>
				(await api.request<any>("GET", "/api/v2/bootstrap", undefined, false))
					.hardware_connected,
		)
		.toBe(true);
	return hardware;
}

async function disconnectHardware(hardware: any) {
	await hardware.close();
}
