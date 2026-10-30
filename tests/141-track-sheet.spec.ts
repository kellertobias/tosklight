import type { Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { clearProgrammerValues } from "./bench/programmer/programmerValues";
import { command, loadCanonicalCopy, objects } from "./support/catalog";

interface StoredChange {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value?: number } | null;
	automatic_restore?: boolean;
}

interface StoredCueList {
	id: string;
	pool_number?: number | null;
	cues: Array<{ number: string; changes: StoredChange[] }>;
}

async function cueList(api: ApiDriver, pool: number) {
	const list = (await objects<StoredCueList>(api, "cue_list")).find((item) => item.body.pool_number === pool);
	expect(list).toBeDefined();
	return list!.body;
}

const intensity = (list: StoredCueList, cue: number, fixtureId: string) =>
	list.cues[cue].changes.find((change) => change.fixture_id === fixtureId && change.attribute === "intensity" && !change.automatic_restore)?.value?.value;

async function openTrackSheet(page: Page) {
	const dockMode = page.getByRole("button", { name: "Desktops / Built-ins", exact: true });
	if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") await dockMode.click();
	await page.getByRole("button", { name: /New desktop/ }).click();
	await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	await dialog.getByRole("tab", { name: "Playback & Automation" }).click();
	await dialog.getByRole("button").filter({ has: page.getByText("Track Sheet", { exact: true }) }).first().click();
	const pane = page.locator(".desk-pane").filter({ has: page.locator(".track-sheet-window") });
	await expect(pane).toBeVisible();
	await expect(pane).toContainText("Track Sheet");
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	const settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings.getByRole("button", { name: "Maximize pane", exact: true }).click();
	await expect(pane).toHaveAttribute("aria-expanded", "true");
	await page.keyboard.press("Escape");
	await expect(settings).toBeHidden();
	return pane.locator(".track-sheet-window");
}

test("TL-855 @ui › Track Sheet shows tracking states and edits like command-line Update", async ({ api, bench, desk, page }) => {
	test.setTimeout(120_000);
	const show = await loadCanonicalCopy(api, bench, "track-sheet");
	const clear = () => clearProgrammerValues(api, { surface: "api", showId: show.id });
	// Two identical Cuelists: one edited from the Track Sheet, one with the command line.
	for (const pool of [141, 142]) {
		await command(api, "FIXTURE 1 AT 50");
		await command(api, `RECORD CUELIST ${pool}`);
		await clear();
		await command(api, "FIXTURE 2 AT 100");
		await command(api, `RECORD CUELIST ${pool}`);
		await clear();
		await command(api, "FIXTURE 1 AT 20");
		await command(api, `RECORD CUELIST ${pool}`);
		await clear();
	}
	const before = await cueList(api, 141);
	const fixtureOne = before.cues[0].changes[0].fixture_id;

	await page.setViewportSize({ width: 1400, height: 900 });
	await desk.open(api.baseUrl);
	const sheet = await openTrackSheet(page);
	await sheet.getByRole("button", { name: "Cuelist", exact: true }).click();
	await page.getByRole("option", { name: /^141 / }).click();
	const cell = (cueNumber: string, state: string) =>
		sheet.getByRole("gridcell", { name: new RegExp(`, Cue ${cueNumber}: ${state}`) });
	await expect(cell("1", "New value 50%").first()).toHaveAttribute("data-state", "hard");
	await expect(cell("2", "Tracked 50%").first()).toHaveAttribute("data-state", "tracked");
	await expect(cell("3", "Moves down 20%").first()).toHaveAttribute("data-state", "down");
	await expect(cell("2", "New value 100%").first()).toBeVisible();

	// Filtering by attribute family removes every row of another family.
	await sheet.getByRole("button", { name: "Attributes", exact: true }).click();
	await page.getByRole("option", { name: "Color", exact: true }).click();
	await expect(sheet.getByText("No stored values match these filters.")).toBeVisible();
	await sheet.getByRole("button", { name: "Attributes", exact: true }).click();
	await page.getByRole("option", { name: "Dimmer", exact: true }).click();

	// Edit the stored 50% in Cue 1 to 70% from the sheet...
	await cell("1", "New value 50%").first().click();
	const editor = sheet.getByRole("region", { name: "Track sheet cell" });
	await editor.getByRole("textbox", { name: "Value in percent" }).fill("70");
	await editor.getByRole("button", { name: "Update Cue", exact: true }).click();
	await expect(editor.getByRole("status")).toContainText("Stored 70% in Cue 1");
	await expect(cell("2", "Tracked 70%").first()).toBeVisible();

	// ...and the same change with command-line Update on the twin Cuelist.
	await command(api, "FIXTURE 1 AT 70");
	await command(api, "UPDATE CUELIST 142 CUE 1");
	await clear();
	const edited = await cueList(api, 141);
	const updated = await cueList(api, 142);
	expect(intensity(edited, 0, fixtureOne)).toBeCloseTo(0.7);
	expect(intensity(edited, 0, fixtureOne)).toBeCloseTo(intensity(updated, 0, fixtureOne)!);
	const summary = (list: StoredCueList) =>
		list.cues.map((cue) => cue.changes.filter((change) => !change.automatic_restore).map((change) => [change.fixture_id, change.attribute, change.value]));
	expect(summary(edited)).toEqual(summary(updated));

	// Cue Only on the tracked Cue 2 value restores 70% in Cue 3's place only where Cue 3 stores nothing.
	await cell("2", "Tracked 70%").first().click();
	await editor.getByRole("textbox", { name: "Value in percent" }).fill("10");
	await editor.getByRole("button", { name: "Cue Only", exact: true }).click();
	await expect(editor.getByRole("status")).toContainText("Cue 2 only");
	await expect.poll(async () => intensity(await cueList(api, 141), 1, fixtureOne)).toBeCloseTo(0.1);
	expect(intensity(await cueList(api, 141), 2, fixtureOne)).toBeCloseTo(0.2);
});
