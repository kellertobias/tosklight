import type {
	RuntimeChangeLeadTime,
	RuntimeChangeLeadTimeUpdateOutcome,
	RuntimePerformanceDiagnosticsSnapshot,
} from "../apps/light-desktop/src/api/generated/light-wire";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	objects,
	putObject,
} from "./support/catalog";

test.setTimeout(180_000);
test.use({ viewport: { width: 1600, height: 1100 } });

const UPDATE = "/api/v2/output/change-lead-time/update";

/**
 * docs/testing/40-dmx-change-lead-time.md: the change lead time runs from the instant a Cue
 * should start outputting to the instant the output frame that first carries it is sent. The
 * test bench freezes the desk clock, so a frame rendered N ms after a GO measures exactly N ms.
 */
async function installOneCuePlayback(api: ApiDriver, bench: any, name: string) {
	await loadCanonicalCopy(api, bench, name, "compact-rig");
	const fixtures = await fixtureIdsByNumber(api);
	const existingGroup = (await objects<any>(api, "group")).find(
		(group) => group.id === "1",
	);
	await putObject(
		api,
		"group",
		"1",
		{
			...(existingGroup?.body ?? {}),
			id: "1",
			name: "Group 1",
			fixtures: [1, 2, 3, 4].map((number) => fixtures[number]),
			derived_from: null,
			frozen_from: null,
			programming: existingGroup?.body.programming ?? {},
		},
		existingGroup?.revision ?? 0,
	);
	const cueListId = crypto.randomUUID();
	await putObject(api, "cue_list", cueListId, {
		id: cueListId,
		name: "Lead",
		priority: 0,
		mode: "sequence",
		looped: true,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues: [1, 2].map((number) => ({
			id: crypto.randomUUID(),
			number,
			name: `Cue ${number}`,
			changes: [],
			group_changes: [
				{
					group_id: "1",
					attribute: "intensity",
					value: { kind: "normalized", value: number === 1 ? 1 : 0.5 },
					fade_millis: 0,
					delay_millis: 0,
				},
			],
			fade_millis: 0,
			delay_millis: 0,
			trigger: { type: "manual" },
		})),
	});
	const existingPlayback = (await objects<any>(api, "playback")).find(
		(playback) => playback.id === "1",
	);
	await putObject(
		api,
		"playback",
		"1",
		{
			number: 1,
			name: "Lead Cuelist",
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go", "go_minus", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: false,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
			presentation_icon: null,
			presentation_image: null,
		},
		existingPlayback?.revision ?? 0,
	);
	// Settle every start the setup itself caused, then measure from a clean slate.
	await bench.tick(25);
	await reset(api);
}

async function readChangeLead(api: ApiDriver): Promise<RuntimeChangeLeadTime> {
	const snapshot = await api.request<RuntimePerformanceDiagnosticsSnapshot>(
		"GET",
		"/api/v2/diagnostics/performance",
	);
	return snapshot.output.change_lead;
}

async function reset(api: ApiDriver, requestId = crypto.randomUUID()) {
	return api.request<RuntimeChangeLeadTimeUpdateOutcome>("POST", UPDATE, {
		request_id: requestId,
		reset: true,
	});
}

test("CHANGE-LEAD-001 @api the longest GO-to-sent-frame lead is reported and reset", async ({
	api,
	bench,
}) => {
	await installOneCuePlayback(api, bench, "change-lead-001");
	const cleared = await readChangeLead(api);
	expect(cleared).toMatchObject({
		maximum_micros: null,
		recent_maximum_micros: null,
		last_micros: null,
		samples: 0,
		recent_window_seconds: 60,
		plausible_limit_micros: 5_000_000,
	});

	// GO, then the first frame leaves 40 ms later: a 40 ms lead.
	await api.playbackNumberAction(1, "go", {});
	const first = await bench.tick(40);
	expect(
		first.universes.find((universe: any) => universe.universe === 1)?.slots
			.slice(0, 4),
	).toEqual([255, 255, 255, 255]);
	expect(await readChangeLead(api)).toMatchObject({
		maximum_micros: 40_000,
		recent_maximum_micros: 40_000,
		last_micros: 40_000,
		samples: 1,
	});

	// A quicker GO keeps the longest as the maximum and reports itself as the latest.
	await api.playbackNumberAction(1, "go", {});
	await bench.tick(15);
	// Frames that carry no start measure nothing.
	await bench.tick(25);
	expect(await readChangeLead(api)).toMatchObject({
		maximum_micros: 40_000,
		recent_maximum_micros: 40_000,
		last_micros: 15_000,
		samples: 2,
	});

	// The recent window forgets a lead older than 60 s; the maximum keeps it until a reset.
	await bench.tick(61_000);
	expect(await readChangeLead(api)).toMatchObject({
		maximum_micros: 40_000,
		recent_maximum_micros: null,
	});

	const requestId = crypto.randomUUID();
	const outcome = await reset(api, requestId);
	expect(outcome.replayed).toBe(false);
	expect(outcome.change_lead).toMatchObject({
		maximum_micros: null,
		last_micros: null,
		samples: 0,
	});

	// A resent reset answers with the first outcome and never wipes a lead measured since.
	await api.playbackNumberAction(1, "go", {});
	await bench.tick(20);
	const replayed = await reset(api, requestId);
	expect(replayed.replayed).toBe(true);
	expect(await readChangeLead(api)).toMatchObject({
		maximum_micros: 20_000,
		samples: 1,
	});
});

test("CHANGE-LEAD-002 @ui the DMX output summary shows the change lead time and resets it", async ({
	page,
	desk,
	api,
	bench,
}) => {
	page.setDefaultTimeout(12_000);
	await installOneCuePlayback(api, bench, "change-lead-002");
	await desk.open(api.baseUrl);
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== "desks")
		await toggle.click();
	await page.getByRole("button", { name: /New desktop/ }).click();
	await expect(page.locator(".empty-desk")).toBeVisible();
	await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText("DMX output", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			break;
		}
	}

	const lead = page
		.locator(".desk-pane .dmx-info-pane")
		.getByRole("region", { name: "Change lead time" });
	await expect(lead).toBeVisible();
	await expect(lead.getByRole("definition")).toHaveText(["—", "—", "—"]);

	await api.playbackNumberAction(1, "go", {});
	await bench.tick(30);
	// The summary reads the desk's live readings once a second.
	await expect(lead.getByRole("definition")).toHaveText([
		"30.0 ms",
		"30.0 ms",
		"30.0 ms",
	]);
	await expect(lead).toContainText("Last 60 s");

	const resetButton = lead.getByRole("button", { name: "Reset" });
	const box = await resetButton.boundingBox();
	// A desk touch target, not a desktop-sized link.
	expect(box?.height ?? 0).toBeGreaterThanOrEqual(40);
	await resetButton.click();
	await expect(lead.getByRole("definition")).toHaveText(["—", "—", "—"]);
	expect(await readChangeLead(api)).toMatchObject({
		maximum_micros: null,
		samples: 0,
	});
});
