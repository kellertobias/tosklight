import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

test("GROUP-SELECTION @ui › tiles distinguish live Groups from full and partial fixture selection", async ({ api, bench, desk, page, show }, testInfo) => {
	testInfo.setTimeout(90_000);
	const t = new BrowserScenarioWorld(page, desk, bench, api, show, testInfo);
	try {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("Group selection indicators");
		desktop.addPane(PaneType.Groups, { slug: "groups", column: 1, row: 1, width: 24, height: 18 });
		await desktop.apply();
		const card = (id: number) => page.locator(`.group-pool-window .group-card[data-pool-slot-id="${id}"]`);
		await t.command.execute("FIXTURE 1");
		await t.group.via.api.store(10, { mode: StoreMode.Overwrite });
		await t.group.expect(10).fixtures(1);
		await t.selection.clear();
		await card(1).click();
		await expect(card(1)).toHaveAttribute("data-group-selection", "group");
		await expect(card(1)).toHaveClass(/selected/);
		const selectedBackground = await card(1).evaluate((tile) => getComputedStyle(tile).backgroundColor);
		await t.command.execute("GROUP 1 + GROUP 2");
		await expect(card(1)).toHaveAttribute("data-group-selection", "group");
		await expect(card(2)).toHaveAttribute("data-group-selection", "group");
		await expect(card(2)).toHaveClass(/selected/);
		await expect(card(10)).toHaveAttribute("data-group-selection", "full");
		await expect(card(10)).not.toHaveClass(/selected/);
		const dot = card(10).getByRole("img", { name: "All group fixtures selected; group not selected" });
		await expect(dot).toBeVisible();
		const dotStyle = await dot.evaluate((node) => ({
			color: getComputedStyle(node).backgroundColor,
			left: node.getBoundingClientRect().left - node.closest("button")!.getBoundingClientRect().left,
			bottom: node.closest("button")!.getBoundingClientRect().bottom - node.getBoundingClientRect().bottom,
		}));
		expect(dotStyle.color).toBe("rgb(255, 255, 255)");
		expect(dotStyle.left).toBeGreaterThanOrEqual(8);
		expect(dotStyle.left).toBeLessThanOrEqual(10);
		expect(dotStyle.bottom).toBeGreaterThanOrEqual(2);
		expect(dotStyle.bottom).toBeLessThanOrEqual(4);
		await page.screenshot({ path: testInfo.outputPath("groups-full-selection.png") });
		await card(1).dblclick();
		await expect(card(1)).toHaveAttribute("data-group-selection", "full");
		await expect(card(1)).toHaveAttribute("aria-pressed", "false");
		await expect(card(1)).not.toHaveClass(/selected/);
		await expect(card(1).getByRole("img", { name: "All group fixtures selected; group not selected" })).toBeVisible();
		await t.command.execute("FIXTURE 1");
		await expect(card(1)).toHaveAttribute("data-group-selection", "partial");
		await expect(card(1).getByRole("img", { name: "Some group fixtures selected; group not selected" })).toBeVisible();
		await expect(card(1)).not.toHaveClass(/selected/);
		await page.screenshot({ path: testInfo.outputPath("groups-partial-selection.png") });
		expect(await card(1).evaluate((tile) => getComputedStyle(tile).backgroundColor)).not.toBe(selectedBackground);
		await t.command.execute("DEGROUP 1");
		await expect(card(1)).toHaveAttribute("data-group-selection", "full");
		await expect(card(1)).not.toHaveClass(/selected/);
		await t.selection.clear();
		await expect(page.locator('.group-pool-window [data-group-selection="group"], .group-pool-window [data-group-selection="full"], .group-pool-window [data-group-selection="partial"]')).toHaveCount(0);
		await expect(card(4)).toHaveAttribute("data-group-selection", "none");
	} finally {
		await t.finish();
	}
});
