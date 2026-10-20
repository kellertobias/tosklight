import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";
import { BrowserDesktops } from "./bench/window-system/desktopScenario";
import { PaneType } from "./bench/window-system/paneTypes";

async function settings(page: Page, pane: Locator) {
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	const dialog = page.getByRole("dialog", { name: "Pane Settings" });
	const tab = dialog.getByRole("tab", { name: "Pane Settings", exact: true });
	if (await tab.count()) await tab.click();
	await expect(dialog.getByLabel("Pane title", { exact: true })).toBeVisible();
	return dialog;
}

async function expectGrid(pane: Locator, rows: number, columns: number) {
	await expect.poll(async () => pane.locator(".pool-window-grid").evaluate((grid, { rows, columns }) => {
		const cells = [...grid.children];
		const first = cells[0].getBoundingClientRect();
		const last = cells[rows * columns - 1].getBoundingClientRect();
		const viewport = grid.closest(".ui-window-scroller")!.getBoundingClientRect();
		const style = getComputedStyle(grid);
		return Math.abs(first.top - viewport.top) <= 2.5 &&
			Math.abs(last.bottom - viewport.bottom) <= 2.5 &&
			style.gridTemplateColumns.split(" ").length === columns;
	}, { rows, columns })).toBe(true);
}

test("PANE-001 @ui pane titles and square/fixed tile layouts are available in the catalog and survive a real server restart", async ({ page, api, bench, desk }, testInfo) => {
	testInfo.setTimeout(120_000);
	page.setDefaultTimeout(10_000);
	await desk.open(bench.baseUrl);
	const desktops = new BrowserDesktops(page, async (name, body) => { await testInfo.attach(name, { body, contentType: "image/png" }); });
	const layout = desktops.configure("Pool Presentation");
	const kinds = [PaneType.Groups, PaneType.Presets, PaneType.Cuelists, PaneType.Dynamics, PaneType.Macros, PaneType.Timecode];
	const panes = kinds.map((kind, index) => layout.addPane(kind, {
		slug: `presentation-${kind}`, column: index % 2 * 12 + 1, row: Math.floor(index / 2) * 6 + 1, width: 12, height: 6,
	}));
	await layout.apply();
	for (const [index, handle] of panes.entries()) {
		const pane = handle.root();
		await expect(pane.locator(".pool-window-grid")).toBeVisible();
		const first = pane.locator(".pool-window-grid > *").first();
		await expect.poll(async () => { const b = await first.boundingBox(); return b ? Math.abs(b.width - b.height) : 999; }).toBeLessThan(2);
		const dialog = await settings(page, pane);
		await dialog.getByLabel("Pane title", { exact: true }).fill(`Tour ${kinds[index]}`);
		await dialog.getByRole("radio", { name: "Fixed grid", exact: true }).click();
		await dialog.getByLabel("Tile rows").fill("3");
		await dialog.getByLabel("Tile columns").fill("4");
		await dialog.getByRole("button", { name: "Close settings", exact: true }).click();
		await expect(pane.locator("header").first()).toContainText(`Tour ${kinds[index]}`);
		await expectGrid(pane, 3, 4);
	}
	const saved = async () => {
		const snapshot = await api.request<any>("GET", "/api/v2/objects/user_layout/desk");
		return snapshot.object?.body?.desks?.find((d: any) => d.name === "Pool Presentation")?.panes ?? [];
	};
	await expect.poll(async () => (await saved()).filter((p: any) => p.titleIsCustom && p.poolTileLayout?.rows === 3 && p.poolTileLayout?.columns === 4).length).toBe(6);
	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden();
	for (const [index, handle] of panes.entries()) {
		await expect(handle.root().locator("header").first()).toContainText(`Tour ${kinds[index]}`);
		await expectGrid(handle.root(), 3, 4);
	}
	const dialog = await settings(page, panes[0].root());
	await dialog.getByRole("radio", { name: "Automatic squares", exact: true }).click();
	await dialog.getByRole("button", { name: "Close settings", exact: true }).click();
	await expect.poll(async () => { const b = await panes[0].root().locator(".pool-window-grid > *").first().boundingBox(); return b ? Math.abs(b.width - b.height) : 999; }).toBeLessThan(2);
});
