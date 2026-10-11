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
		const group1 = await t.group.object(1);
		expect(group1).not.toBeNull();
		const group1Total = group1!.body.fixtures.length;
		expect(group1Total).toBeGreaterThan(1);
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
		// Membership is shown by the existing bottom-left count, independently of live Group selection.
		const count = card(10).locator(".group-fixture-count-selected");
		await expect(count).toHaveText("1");
		await expect(card(10)).toHaveAttribute("data-group-membership", "full");
		const countStyle = await count.evaluate((node) => {
			const bounds = node.getBoundingClientRect();
			const cardBounds = node.closest("button")!.getBoundingClientRect();
			return {
				color: getComputedStyle(node).color,
				inside: bounds.left >= cardBounds.left && bounds.right <= cardBounds.right
					&& bounds.top >= cardBounds.top && bounds.bottom <= cardBounds.bottom,
				lowerLeft: bounds.left < cardBounds.left + cardBounds.width / 2
					&& bounds.bottom > cardBounds.top + cardBounds.height / 2,
			};
		});
		expect(countStyle.color).toBe("rgb(255, 255, 255)");
		expect(countStyle.inside).toBe(true);
		expect(countStyle.lowerLeft).toBe(true);
		await page.screenshot({ path: testInfo.outputPath("groups-full-selection.png") });
		await card(1).dblclick();
		await expect(card(1)).toHaveAttribute("data-group-selection", "full");
		await expect(card(1)).toHaveAttribute("aria-pressed", "false");
		await expect(card(1)).not.toHaveClass(/selected/);
		await expect(card(1)).toHaveAttribute("data-group-membership", "full");
		await expect(card(1).locator(".group-fixture-count-selected")).toHaveText(String(group1Total));
		await t.command.execute("FIXTURE 1");
		await expect(card(1)).toHaveAttribute("data-group-selection", "partial");
		await expect(card(1)).toHaveAttribute("data-group-membership", "partial");
		const partialCount = card(1).locator(".group-fixture-count");
		const selectedCount = partialCount.locator(".group-fixture-count-selected");
		await expect(partialCount).toHaveText(`1/${group1Total}`);
		await expect(selectedCount).toHaveText("1");
		expect(await selectedCount.evaluate((node) => getComputedStyle(node).color)).toBe("rgb(255, 255, 255)");
		expect(await partialCount.evaluate((node) => getComputedStyle(node).color)).not.toBe("rgb(255, 255, 255)");
		await expect(card(1)).not.toHaveClass(/selected/);
		await page.screenshot({ path: testInfo.outputPath("groups-partial-selection.png") });
		expect(await card(1).evaluate((tile) => getComputedStyle(tile).backgroundColor)).not.toBe(selectedBackground);
		await t.command.execute("DEGROUP 1");
		await expect(card(1)).toHaveAttribute("data-group-selection", "full");
		await expect(card(1)).not.toHaveClass(/selected/);
		await t.selection.clear();
		await expect(page.locator('.group-pool-window [data-group-selection="group"], .group-pool-window [data-group-selection="full"], .group-pool-window [data-group-selection="partial"]')).toHaveCount(0);
		await expect(card(4)).toHaveAttribute("data-group-selection", "none");
		await expect(card(1).locator(".group-fixture-count-selected")).toHaveCount(0);
		expect(await card(1).locator(".group-fixture-count").evaluate((node) => getComputedStyle(node).color)).not.toBe("rgb(255, 255, 255)");
	} finally {
		await t.finish();
	}
});
