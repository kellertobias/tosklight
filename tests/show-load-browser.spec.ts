import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

interface ShowRecord {
	id: string;
	name: string;
	revision: number;
	revision_copy: { show_id: string; revision: number } | null;
}

async function openBrowser(page: Page) {
	const menu = page.getByRole("dialog", { name: "Show", exact: true });
	if (!(await menu.isVisible())) await page.getByRole("button", { name: /Open show menu/ }).click();
	await menu
		.getByRole("button", { name: "Load", exact: true }).click();
	const browser = page.getByRole("dialog", { name: "Load show", exact: true });
	await expect(browser).toBeVisible();
	return browser;
}

async function chooseSource(page: Page, scope: Locator, next: string) {
	await scope.getByRole("button", { name: "Source", exact: true }).click();
	await page.getByRole("menuitem", { name: next, exact: true }).click();
}

for (const hardwareConnected of [false, true]) {
	test(`TL-542 @ui › folder browser and revision actions preserve source shows (${hardwareConnected ? "hardware connected" : "software only"})`, async ({ api, bench, desk, page, show }) => {
		test.setTimeout(90_000);
		await page.setViewportSize({ width: 1024, height: 600 });
		const name = `TL-542 source ${crypto.randomUUID()}`;
		const source = await api.createShow<ShowRecord>({ name });
		await api.openShow(source.id, { transition: "hold_current" });
		const named = await api.saveShowRevision<{ revision: number }>(source.id, "Operator checkpoint");
		await api.openShow(show.id, { transition: "hold_current" });
		const original = (await api.shows<ShowRecord>()).find(item => item.id === source.id)!;
		const active = async () => (await api.request<{ active_show: ShowRecord }>("GET", "/api/v2/bootstrap", undefined, false)).active_show;
		const hardware = hardwareConnected ? await bench.osc() : null;
		try {
			if (hardware) await hardware.subscribe(`tl-542-${crypto.randomUUID()}`, "desk");
			await expect.poll(async () => (await api.request<{ hardware_connected: boolean }>("GET", "/api/v2/bootstrap", undefined, false)).hardware_connected).toBe(hardwareConnected);
			await desk.open(api.baseUrl);
			let browser = await openBrowser(page);
			await expect(browser.getByRole("button", { name: "Close Load Show" })).toBeInViewport();
			await expect(browser.getByRole("button", { name: /MVR/ })).toHaveCount(0);
			const folder = `TL-542 folder ${crypto.randomUUID()}`;
			await expect(browser.getByRole("button", { name: "Create New Folder" })).toHaveCount(0);
			await api.request("POST", "/api/v2/files/shows/operations", { operation: "create_folder", sources: [], destination: "", name: folder });
			await browser.getByRole("button", { name: "Close Load Show" }).click();
			browser = await openBrowser(page);
			await browser.getByRole("button", { name: `📁 ${folder}`, exact: true }).click();
			await expect(browser.locator(".show-browser-path")).toContainText(folder);
			await browser.getByRole("button", { name: "Up one folder" }).click();
			await chooseSource(page, browser, "USB");
			await expect(browser.locator(".show-browser-path")).not.toContainText(folder);
			await chooseSource(page, browser, "Network");
			await expect(browser.getByRole("button", { name: "Create New Folder" })).toHaveCount(0);
			await chooseSource(page, browser, "Internal");
			const row = () => browser.getByRole("row").filter({ has: page.getByText(name, { exact: true }) });
			await row().getByRole("button", { name: "Load Latest", exact: true }).click();
			await expect(browser).toBeHidden();
			await expect.poll(async () => (await active()).id).toBe(source.id);
			await api.openShow(show.id, { transition: "hold_current" });
			browser = await openBrowser(page);
			await row().getByRole("button", { name: `Revisions for ${name}`, exact: true }).click();
			let revisions = page.getByRole("dialog", { name: `Revisions for ${name}`, exact: true });
			await expect(revisions).toBeVisible();
			await expect(revisions.locator("tbody tr").first()).toContainText("Latest Autosave");
			const namedRow = revisions.getByRole("row").filter({ hasText: `Revision ${named.revision} · Operator checkpoint` });
			await expect(namedRow.getByRole("button", { name: "Partial Load", exact: true })).toBeInViewport();
			await namedRow.getByRole("button", { name: "Load", exact: true }).click();
			await expect(browser).toBeHidden();
			await expect.poll(async () => (await active()).revision_copy).toMatchObject({ show_id: source.id, revision: named.revision });
			expect((await active()).id).not.toBe(source.id);
			for (const useNamed of [false, true]) {
				await api.openShow(show.id, { transition: "hold_current" });
				browser = await openBrowser(page);
				await row().getByRole("button", { name: `Revisions for ${name}`, exact: true }).click();
				revisions = page.getByRole("dialog", { name: `Revisions for ${name}`, exact: true });
				const revisionRow = revisions.getByRole("row").filter({ hasText: useNamed ? `Revision ${named.revision} · Operator checkpoint` : "Latest Autosave" });
				await revisionRow.getByRole("button", { name: "Partial Load", exact: true }).click();
				const partial = page.getByRole("dialog", { name: "Partial Show Load", exact: true });
				await expect(partial).toBeVisible();
				await expect(partial.getByRole("button", { name: "Source show", exact: true })).not.toContainText("Choose a show");
				expect((await active()).id).toBe(show.id);
				await partial.getByRole("button", { name: "Cancel", exact: true }).click();
				await expect(partial).toBeHidden();
			}
			const after = (await api.shows<ShowRecord>()).find(item => item.id === source.id)!;
			expect(after.revision).toBe(original.revision);
			expect(after.name).toBe(original.name);
			expect((await api.showRevisions<{ revision: number; name: string }>(source.id))).toContainEqual(expect.objectContaining({ revision: named.revision, name: "Operator checkpoint" }));
			const menu = page.getByRole("dialog", { name: "Show", exact: true });
			if (!(await menu.isVisible())) await page.getByRole("button", { name: /Open show menu/ }).click();
			await menu.getByRole("button", { name: "Save As", exact: true }).click();
			const save = page.getByRole("dialog", { name: "Save show", exact: true });
			await expect(save.locator(".ui-modal-titlebar").getByRole("button", { name: "Source", exact: true })).toBeInViewport();
			await save.getByRole("button", { name: `📁 ${folder}`, exact: true }).click();
			const copyName = "Operator folder copy";
			await save.getByRole("textbox", { name: "Show name", exact: true }).fill(copyName);
			const base = save.getByRole("button", { name: "Save as a base show", exact: true });
			await expect(base).toHaveAttribute("aria-pressed", "false");
			await base.click();
			await expect(base).toHaveAttribute("aria-pressed", "true");
			await save.getByRole("button", { name: "Save as New Show", exact: true }).click();
			await expect(save.getByRole("status").filter({ hasText: `Saved ${copyName}` })).toBeVisible();
			expect((await active()).id).toBe(show.id);
			const directory = () => api.request<{ entries: Array<{ name: string }> }>("GET", `/api/v2/files/shows/entries?path=${encodeURIComponent(folder)}`);
			await expect.poll(async () => (await directory()).entries.map(item => item.name)).toContain(`${copyName}.show`);
			await save.getByRole("button", { name: "Export MVR", exact: true }).click();
			await expect(save.getByRole("status").filter({ hasText: "Exported MVR" })).toBeVisible();
			await expect.poll(async () => (await directory()).entries.map(item => item.name)).toContain(`${copyName}.mvr`);
			expect((await active()).id).toBe(show.id);
		} finally {
			await hardware?.close();
		}
	});
}
