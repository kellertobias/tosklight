import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

const PATCH_HEADERS = [
	"Type",
	"Fixture ID",
	"Name",
	"Fixture / mode",
	"Patch",
	"Masters",
	"Pan / Tilt",
	"MIB",
	"Light source",
	"Location X",
	"Location Y",
	"Location Z",
	"Rotation X",
	"Rotation Y",
	"Rotation Z",
	"Layer",
];

test("PATCH-APPEARANCE-003-001 @ui › exact table, combined MIB, and emitterless source state survive reopen", async ({
	api,
	bench,
	desk,
	page,
}) => {
	const show = await loadCanonicalCopy(
		api,
		bench,
		"patch-appearance-003-001",
		"default-stage",
	);
	await desk.open(api.baseUrl);
	await openPatch(page);

	await expect(page.locator(".patch-table thead th")).toHaveText(PATCH_HEADERS);
	await expect(page.locator(".patch-table thead")).not.toContainText(
		/MIB Delay|Group Masters|Grand Master|Invert Pan|Invert Tilt/,
	);

	const mib = page.getByRole("button", { name: /^MIB 101:/ });
	await expect(mib).toHaveText("0 s");
	await mib.click();
	await expect(page.locator(".patch-edit-modal")).toHaveCount(0);
	await page.getByRole("button", { name: "SET", exact: true }).click();
	await mib.click();
	const mibEditor = page.locator(".patch-edit-modal");
	await mibEditor
		.getByLabel("MIB value: Off or non-negative seconds")
		.fill("Off");
	await mibEditor.getByRole("button", { name: "Set", exact: true }).click();
	await expect(
		page.getByRole("button", { name: "MIB 101: Off" }),
	).toBeVisible();
	await expect(mib.locator("xpath=ancestor::tr")).toContainText(
		"UnavailableNo geometry emitter",
	);

	await expect
		.poll(async () => {
			const fixture = (await api.patch()).fixtures.find(
				(candidate) => candidate.fixture_number === 101,
			);
			return fixture
				? {
						mib: fixture.move_in_black_enabled,
						delay: fixture.move_in_black_delay_millis,
					}
				: null;
		})
		.toEqual({
			mib: false,
			delay: 0,
		});

	await api.openShow(show.id, { transition: "hold_current" });
	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden({
		timeout: 10_000,
	});
	await openPatch(page);
	await expect(
		page.getByRole("button", { name: "MIB 101: Off" }),
	).toBeVisible();
});

async function openPatch(page: import("@playwright/test").Page) {
	if (await page.locator(".patch-window").isVisible()) return;
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await expect(page.locator(".patch-window")).toBeVisible();
}
