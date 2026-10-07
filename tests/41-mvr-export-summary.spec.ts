import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";
import { saveProfile } from "./bench/show-setup/fixtureMappingEditorScenario";
import { gdtfProbeProfile, openFreshShow, patchProfile } from "./bench/show-setup/gdtfTransferScenario";

const GENERATED_WARNING = "ToskLight generated GDTF files from the current fixture profiles";

async function openSaveAs(page: Page) {
	const menu = page.getByRole("dialog", { name: "Show", exact: true });
	if (!(await menu.isVisible())) await page.getByRole("button", { name: /Open show menu/ }).click();
	await menu.getByRole("button", { name: "Save As", exact: true }).click();
	const save = page.getByRole("dialog", { name: "Save show", exact: true });
	await expect(save).toBeVisible();
	return save;
}

async function expectTouchSized(control: Locator) {
	const box = await control.boundingBox();
	if (!box) throw new Error("Expected a rendered control");
	expect(box.height).toBeGreaterThanOrEqual(44);
}

test.describe("docs/testing/41-mvr-export-summary.md", () => {
	test("MVR-EXPORT-001 @ui › one press exports and then shows the server's summary and every warning", async ({ api, desk, page }) => {
		const profile = gdtfProbeProfile(`MVR Summary ${crypto.randomUUID().slice(0, 6)}`);
		await saveProfile(api, profile);
		const showId = await openFreshShow(api, "mvr-summary");
		await patchProfile(api, showId, { id: profile.id, revision: 1, modeId: profile.modes[0].id, footprint: 3 }, 2);
		await desk.open(api.baseUrl);
		const save = await openSaveAs(page);
		const name = `MVR summary ${crypto.randomUUID().slice(0, 8)}`;
		await save.getByRole("textbox", { name: "Show name", exact: true }).fill(name);
		await save.getByRole("button", { name: "Export MVR", exact: true }).click();

		const summary = save.getByRole("status", { name: "MVR export summary" });
		await expect(summary).toContainText(`Exported MVR to Shows / ${name}.mvr`);
		await expect(summary).toContainText("2 fixtures · 0 scenery objects");
		await expect(summary).toContainText("Not included: cues, presets, playbacks, users, and desk layouts");
		const warnings = summary.getByRole("list", { name: "MVR export warnings" }).getByRole("listitem");
		await expect(warnings.filter({ hasText: GENERATED_WARNING })).toHaveCount(1);
		await expect(summary).toContainText(new RegExp(`${await warnings.count()} export warnings?`, "u"));
		const entries = await api.request<{ entries: Array<{ name: string }> }>("GET", "/api/v2/files/shows/entries?path=");
		expect(entries.entries.map((entry) => entry.name)).toContain(`${name}.mvr`);

		await expectTouchSized(summary.getByRole("button", { name: "Copy warnings" }));
		const dismiss = summary.getByRole("button", { name: "Dismiss" });
		await expectTouchSized(dismiss);
		// The summary does not time out on its own.
		await page.waitForTimeout(1_500);
		await expect(summary).toBeVisible();
		await dismiss.click();
		await expect(summary).toHaveCount(0);
	});

	test("MVR-EXPORT-002 @ui › a failed export keeps a copyable error and shows no summary", async ({ api, desk, page }) => {
		const showId = await openFreshShow(api, "mvr-failure");
		const name = `MVR failure ${crypto.randomUUID().slice(0, 8)}`;
		await api.request("POST", "/api/v2/shows", {
			request_id: crypto.randomUUID(),
			action: { type: "export_mvr_file", show_id: showId, data_base64: null, name, root_id: "shows", path: "" },
		});
		await desk.open(api.baseUrl);
		const save = await openSaveAs(page);
		await save.getByRole("textbox", { name: "Show name", exact: true }).fill(name);
		await save.getByRole("button", { name: "Export MVR", exact: true }).click();

		const alert = save.getByRole("alert");
		await expect(alert).toContainText(/already exists/iu);
		await expect(alert.getByRole("button", { name: "Copy error" })).toBeVisible();
		await expect(save.getByRole("status", { name: "MVR export summary" })).toHaveCount(0);
		await expect(save.getByText("Exporting MVR…")).toHaveCount(0);
		await expect(save.getByRole("button", { name: "Export MVR", exact: true })).toBeEnabled();
	});
});
