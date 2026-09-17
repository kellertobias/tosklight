import type { Locator, Page } from "@playwright/test";
import { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import { readPatchSnapshot } from "./support/operator/patch";

const VALID_CSV = [
	"Patch,Fixture ID,Fixture Name,Manufacturer,Fixture Type,Mode",
	"2.1,501,CSV Dimmer,E2E,Generic Dimmer,1ch",
	"99.999,502,Bad Patch,E2E,Generic Dimmer,1ch",
	"2.3,abc,Bad ID,E2E,Generic Dimmer,1ch",
].join("\n");

function csvFile(name: string, text: string) {
	return { name, mimeType: "text/csv", buffer: Buffer.from(text) };
}

function patchHeader(page: Page): Locator {
	return page
		.locator("header.ui-window-header")
		.filter({ hasText: "Show Patch" })
		.first();
}

async function openShowPatch(page: Page) {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await expect(
		patchHeader(page).getByRole("tab", { name: "Fixtures", exact: true }),
	).toHaveAttribute("aria-selected", "true");
}

function importDialog(page: Page) {
	return page.getByRole("dialog", { name: /^Import CSV/ });
}

test("TL-469 @ui › Import CSV lives in Show Patch Settings and keeps validation and safe cancellation", async ({
	api,
	desk,
	page,
	show,
}) => {
	const before = await readPatchSnapshot(api, show.id);
	await desk.open(api.baseUrl);
	await openShowPatch(page);
	const header = patchHeader(page);
	// The Fixtures view no longer carries its own Import CSV action.
	await expect(header.getByRole("button", { name: "Import CSV" })).toHaveCount(
		0,
	);
	await expect(
		header.getByRole("button", { name: "+ Add fixture" }),
	).toBeVisible();

	// Keyboard: focus the ⚙, open Settings, and press Import CSV in its title. Space activates
	// a focused control; Enter stays the desk's ENT key.
	await header.getByRole("button", { name: "Settings", exact: true }).focus();
	await page.keyboard.press("Space");
	const settings = page.getByRole("dialog", { name: "Show Patch" });
	const titleAction = settings
		.locator(".ui-modal-titlebar")
		.getByRole("button", { name: "Import CSV", exact: true });
	await expect(titleAction).toBeVisible();
	await titleAction.focus();
	await page.keyboard.press("Space");
	await expect(settings).toHaveCount(0);
	const dialog = importDialog(page);
	await expect(dialog).toBeVisible();

	// Nothing chosen yet: Close leaves at once.
	await dialog.getByRole("button", { name: "Close Import CSV" }).click();
	await expect(dialog).toHaveCount(0);

	// Mouse: reopen and try an empty file first.
	await header.getByRole("button", { name: "Settings", exact: true }).click();
	await titleAction.click();
	await expect(dialog).toBeVisible();
	const file = dialog.getByLabel("CSV file");
	await file.setInputFiles(csvFile("empty.csv", ""));
	await expect(dialog.getByRole("alert")).toHaveText(
		"empty.csv contains no rows.",
	);
	await expect(
		dialog.getByRole("button", { name: "Next: fixture types" }),
	).toBeDisabled();

	// A chosen file asks before closing; Stay keeps every choice.
	await file.setInputFiles(csvFile("rig.csv", VALID_CSV));
	const assignments = dialog.getByRole("list", { name: "Column assignments" });
	await expect(assignments).toBeVisible();
	await dialog.getByRole("button", { name: "Close Import CSV" }).click();
	await page.getByRole("button", { name: "Stay in Import CSV" }).click();
	await expect(assignments).toBeVisible();
	await dialog.getByRole("button", { name: "Close Import CSV" }).click();
	await page.getByRole("button", { name: "Yes, close" }).click();
	await expect(dialog).toHaveCount(0);
	expect((await readPatchSnapshot(api, show.id)).fixtures).toHaveLength(
		before.fixtures.length,
	);

	// Valid import: invalid rows are reported and left out, the valid one lands once.
	await header.getByRole("button", { name: "Settings", exact: true }).click();
	await titleAction.click();
	await file.setInputFiles(csvFile("rig.csv", VALID_CSV));
	await dialog.getByRole("button", { name: "Next: fixture types" }).click();
	// The file's fixture type is not in the library, so the wizard asks for one first.
	await expect(
		dialog.getByRole("button", { name: /Needs a library fixture/ }),
	).toBeVisible();
	const next = dialog.getByRole("button", { name: "Next: review" });
	await expect(next).toBeDisabled();
	await dialog
		.getByRole("button", { name: /^Dimmer Generic · \d+ modes$/ })
		.click();
	await dialog.getByRole("button", { name: "Use this fixture" }).click();
	await next.click();
	const table = dialog.getByRole("table", { name: "Fixtures to import" });
	await expect(table).toBeVisible();
	await expect(table.getByText("Not imported")).toHaveCount(2);
	await dialog.getByRole("button", { name: "Import 1 fixture" }).click();
	await expect(dialog).toHaveCount(0);
	await expect
		.poll(async () => {
			const snapshot = await readPatchSnapshot(api, show.id);
			return snapshot.fixtures
				.filter((fixture) => fixture.fixture_number === 501)
				.map((fixture) => {
					const profile = snapshot.profile_revisions.find(
						(candidate) => candidate.profile_id === fixture.profile_id,
					);
					return [
						fixture.name,
						profile?.manufacturer,
						profile?.name,
						fixture.split_patches[0]?.universe,
						fixture.split_patches[0]?.address,
					];
				});
		})
		.toEqual([["CSV Dimmer", "Generic", "Dimmer", 2, 1]]);
	const after = await readPatchSnapshot(api, show.id);
	expect(after.fixtures).toHaveLength(before.fixtures.length + 1);
	expect(after.fixtures.some((fixture) => fixture.fixture_number === 502)).toBe(
		false,
	);
});

test("TL-469 @ui @touch › a touch tap reaches Import CSV from Settings on another Show Patch view", async ({
	api,
	bench,
	browser,
	show,
}, testInfo) => {
	expect(show.id).toBeTruthy();
	const context = await browser.newContext({
		baseURL: bench.baseUrl,
		hasTouch: true,
		viewport: { width: 1280, height: 720 },
	});
	const page = await context.newPage();
	const desk = new DeskDriver(
		page,
		testInfo.title,
		api.session?.desk.id ?? null,
	);
	try {
		await desk.open(api.baseUrl);
		await openShowPatch(page);
		const header = patchHeader(page);
		await header.getByRole("tab", { name: "Tracking", exact: true }).tap();
		await expect(
			header.getByRole("tab", { name: "Tracking", exact: true }),
		).toHaveAttribute("aria-selected", "true");
		await header.getByRole("button", { name: "Settings", exact: true }).tap();
		await page
			.getByRole("dialog", { name: "Show Patch" })
			.getByRole("button", { name: "Import CSV", exact: true })
			.tap();
		// Import CSV belongs to Fixtures, so the window switches there and opens it.
		await expect(importDialog(page)).toBeVisible();
		await expect(
			header.getByRole("tab", { name: "Fixtures", exact: true }),
		).toHaveAttribute("aria-selected", "true");
		await importDialog(page)
			.getByRole("button", { name: "Close Import CSV" })
			.tap();
		await expect(importDialog(page)).toHaveCount(0);
	} finally {
		await desk.dispose();
		await context.close();
	}
});
