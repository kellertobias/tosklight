import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

interface PatchRecord {
	fixture_id: string;
	fixture_number: number | null;
	universe: number | null;
	address: number | null;
	split_patches: Array<{ split: number; universe: number | null; address: number | null }>;
}

test("TL-853 @ui › Patch Paperwork lists hookup and DMX sheet, marks unpatched fixtures and exports CSV", async ({ api, bench, desk, page }) => {
	test.setTimeout(90_000);
	await loadCanonicalCopy(api, bench, "patch-paperwork");
	const patch = await api.request<{ patch_revision: number; fixtures: PatchRecord[] }>("GET", "/api/v2/patch");
	const lights = patch.fixtures.filter((fixture) => fixture.fixture_number != null);
	const unpatched = lights.find((fixture) => fixture.fixture_number === 1)!;
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: [{ ...unpatched, universe: null, address: null, split_patches: unpatched.split_patches.map((split) => ({ ...split, universe: null, address: null })) }],
			remove_fixture_ids: [],
		},
		true,
		patch.patch_revision,
	);

	await page.setViewportSize({ width: 1400, height: 900 });
	await desk.open(api.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	const header = page.locator("header.ui-window-header").filter({ hasText: "Show Patch" }).first();
	await header.getByRole("button", { name: "Paperwork", exact: true }).click();
	const dialog = page.getByRole("dialog", { name: "Paperwork", exact: true });
	await expect(dialog).toBeVisible();

	// Channel Hookup: one row per instrument, by fixture ID, fixture 1 marked Unpatched.
	const hookup = dialog.getByRole("table", { name: "Channel Hookup" });
	await expect(hookup.locator("tbody tr")).toHaveCount(lights.length);
	const ids = await hookup.locator("tbody tr td:first-child").allTextContents();
	expect(ids.map(Number)).toEqual([...ids.map(Number)].sort((left, right) => left - right));
	await expect(hookup.locator("tbody tr").first()).toContainText("Unpatched");

	// DMX Sheet: ordered by universe then address, unpatched last.
	await dialog.getByRole("button", { name: "Report", exact: true }).click();
	await page.getByRole("option", { name: "DMX Sheet", exact: true }).click();
	const sheet = dialog.getByRole("table", { name: "DMX Sheet" });
	const rows = await sheet.locator("tbody tr").evaluateAll((elements) =>
		elements.map((row) => [...row.querySelectorAll("td")].map((cell) => cell.textContent ?? "")),
	);
	const patched = rows.filter((row) => row[0] !== "Unpatched");
	const keys = patched.map((row) => Number(row[0]) * 1000 + Number(row[1]));
	expect(keys).toEqual([...keys].sort((left, right) => left - right));
	expect(rows.at(-1)?.[0]).toBe("Unpatched");
	expect(rows.at(-1)?.[3]).toBe("1");

	// Export CSV writes the same rows.
	const download = page.waitForEvent("download");
	await dialog.getByRole("button", { name: "Export CSV", exact: true }).click();
	const file = await download;
	expect(file.suggestedFilename()).toBe("DMX Sheet.csv");
	const csv = await (await file.createReadStream()).toArray();
	const lines = Buffer.concat(csv).toString("utf8").trim().split("\r\n");
	expect(lines[0]).toBe("Universe,Address,End,Fixture,Type,Mode,Split");
	expect(lines).toHaveLength(rows.length + 1);
	await expect(dialog.getByRole("status")).toContainText("Exported DMX Sheet.csv");

	// Symbol Key counts every instrument once.
	await dialog.getByRole("button", { name: "Report", exact: true }).click();
	await page.getByRole("option", { name: "Symbol Key", exact: true }).click();
	const counts = await dialog.getByRole("table", { name: "Symbol Key" }).locator("tbody tr td:nth-child(2)").allTextContents();
	expect(counts.reduce((total, count) => total + Number(count), 0)).toBe(lights.length);
});
