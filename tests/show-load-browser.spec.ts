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
	await scope.getByRole("button", { name: /^Source:/ }).click();
	await page.getByRole("menuitem", { name: next, exact: true }).click();
}

async function expectTableGeometry(dialog: Locator, row: Locator) {
	const frame = await dialog.boundingBox();
	const heading = await dialog.locator(".ui-modal-title-heading").boundingBox();
	const name = await row.locator("td:first-child strong").boundingBox();
	const scroller = await dialog.locator(":scope > .show-browser-table-scroll").boundingBox();
	const actions = row.locator(".show-row-actions .ui-button");
	const last = await actions.last().boundingBox();
	if (!frame || !heading || !name || !scroller || !last) throw new Error("Expected a rendered Load table and header");
	expect(frame.width).toBeGreaterThan(900);
	expect(Math.abs(name.x - heading.x)).toBeLessThanOrEqual(1);
	expect(Math.abs(scroller.x - frame.x)).toBeLessThanOrEqual(1);
	expect(Math.abs(scroller.x + scroller.width - frame.x - frame.width)).toBeLessThanOrEqual(1);
	expect(Math.abs((frame.x + frame.width - last.x - last.width) - (heading.x - frame.x))).toBeLessThanOrEqual(2);
	const first = await actions.first().boundingBox();
	if (!first) throw new Error("Expected row actions");
	expect(Math.abs(first.y - last.y)).toBeLessThanOrEqual(1);
	await expect(dialog.locator(".ui-modal-title-heading")).toBeInViewport();
	expect(await dialog.locator(".ui-modal-title-heading").evaluate(node => node.scrollWidth <= node.clientWidth && node.scrollHeight <= node.clientHeight)).toBe(true);
}

async function expectSharedScrollbar(area: Locator) {
	await expect(area).toHaveClass(/overflowing/);
	await expect(area.locator(".ui-touch-scrollbar")).toBeVisible();
	const scroller = area.locator(".ui-window-scroller");
	expect(await scroller.evaluate(node => node.scrollHeight > node.clientHeight)).toBe(true);
	const track = area.locator(".ui-touch-scrollbar");
	const trackBox = await track.boundingBox();
	if (!trackBox) throw new Error("Expected a visible application scrollbar track");
	await track.click({ position: { x: 8, y: trackBox.height - 4 } });
	await expect.poll(() => scroller.evaluate(node => node.scrollTop)).toBeGreaterThan(0);
	await scroller.evaluate(node => { node.scrollTop = node.scrollHeight; });
	await expect(area.locator("tbody tr").last()).toBeInViewport();
	await scroller.evaluate(node => { node.scrollTop = 0; });
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
			for (let index = 0; index < 12; index++) await api.request("POST", "/api/v2/files/shows/operations", { operation: "create_folder", sources: [], destination: "", name: `Scroll folder ${index}` });
			await browser.getByRole("button", { name: "Close Load Show" }).click();
			browser = await openBrowser(page);
			await expectSharedScrollbar(browser.locator(":scope > .show-browser-table-scroll"));
			await browser.getByRole("button", { name: `📁 ${folder}`, exact: true }).click();
			await expect(browser.locator(".ui-modal-title-heading")).toContainText(folder);
			await browser.getByRole("button", { name: "Up one folder" }).click();
			await browser.getByRole("button", { name: /^Source:/ }).click();
            const drives = page.getByRole("menuitem", { name: /^USB:/ });
            if (await drives.count()) await drives.first().click();
            else {
                await expect(page.getByRole("menuitem", { name: "USB (No drives connected)", exact: true })).toBeDisabled();
                await page.getByRole("menuitem", { name: "Internal", exact: true }).click();
            }
            await expect(browser.getByRole("combobox")).toHaveCount(0);
			await expect(browser.locator(".ui-modal-title-heading")).not.toContainText(folder);
			await chooseSource(page, browser, "Network");
			await expect(browser.getByRole("button", { name: "Create New Folder" })).toHaveCount(0);
			await chooseSource(page, browser, "Internal");
			const row = () => browser.getByRole("row").filter({ has: page.getByText(name, { exact: true }) });
			await expect(row().locator("strong")).toBeVisible();
			await expectTableGeometry(browser, row());
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
			await expect(namedRow.locator("strong")).toBeVisible();
			await expectTableGeometry(revisions, namedRow);
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
			await expect(save.locator(".ui-modal-titlebar").getByRole("button", { name: /^Source:/ })).toBeInViewport();
			const location = save.getByRole("button", { name: /^Location:/ });
			await expect(location).toHaveAttribute("aria-expanded", "false");
			await expect(save.getByRole("button", { name: `📁 ${folder}`, exact: true })).toHaveCount(0);
			await location.click();
			const saveArea = save.locator(".show-save-folder-scroll");
			await expectSharedScrollbar(saveArea);
			const titleX = await save.locator(".ui-modal-title-heading").evaluate(node => node.getBoundingClientRect().x);
			const firstCellX = await saveArea.locator("th").first().evaluate(node => node.getBoundingClientRect().x + Number.parseFloat(getComputedStyle(node).paddingLeft));
			expect(Math.abs(firstCellX - titleX)).toBeLessThanOrEqual(1);
			await save.getByRole("button", { name: `📁 ${folder}`, exact: true }).click();
			const copyName = "Operator folder copy";
			await save.getByRole("textbox", { name: "Show name", exact: true }).fill(copyName);
			const base = save.getByRole("switch", { name: "Save as Template", exact: true });
			await expect(base).not.toBeChecked();
            const template = save.locator(".show-save-template");
            const labelBox = await template.locator(":scope > label[for]").boundingBox();
            const toggleBox = await template.locator(".ui-switch-track").boundingBox();
            const fieldsBox = await save.locator(".show-save-fields").boundingBox();
            if (!labelBox || !toggleBox || !fieldsBox) throw new Error("Template toggle must be visible");
            expect(labelBox.x + labelBox.width).toBeLessThan(toggleBox.x);
            expect(Math.abs(toggleBox.y + toggleBox.height / 2 - labelBox.y - labelBox.height / 2)).toBeLessThanOrEqual(1);
            expect(Math.abs(fieldsBox.x + fieldsBox.width - 16 - toggleBox.x - toggleBox.width)).toBeLessThanOrEqual(1);
			await base.locator("..").click();
			await expect(base).toBeChecked();
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
