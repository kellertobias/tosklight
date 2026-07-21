import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import type {
	Locator,
	Page,
} from "../../apps/control-ui/node_modules/@playwright/test/index.js";

export async function openPlaybackMode(page: Page) {
	if (await page.locator(".playback-fader-bank").isVisible()) return;
	await page.locator(".mode-toggle").click();
	await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

export function playbackCard(page: Page, slotNumber: number): Locator {
	return page.locator(
		`.playback-fader-bank article[data-playback-slot="${slotNumber}"]`,
	);
}

export async function addVirtualPlaybackPane(page: Page): Promise<Locator> {
	await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
	await page.getByRole("button", { name: /New desktop/ }).click();
	const grid = page.locator(".desk-grid");
	const box = await grid.boundingBox();
	expect(box).not.toBeNull();
	await page.mouse.click(
		box!.x + Math.min(120, box!.width / 4),
		box!.y + Math.min(90, box!.height / 4),
	);
	await expect(
		page.getByRole("heading", { name: "Open Window" }),
	).toBeVisible();
	await page
		.getByRole("button", { name: "Virtual Playbacks", exact: true })
		.click();
	return activeVirtualPane(page);
}

export async function activeVirtualPane(page: Page): Promise<Locator> {
	await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
	const activeDesk = page.locator(".dock-list .dock-entry.active");
	if (!(await activeDesk.isVisible().catch(() => false)))
		await page
			.locator(".dock-list .dock-entry")
			.filter({ hasText: /Desk \d+/ })
			.last()
			.click();
	const pane = page
		.locator(".desk-pane")
		.filter({ hasText: "Virtual Playbacks" });
	await expect(pane).toBeVisible();
	return pane;
}

export async function assignVirtualSource(
	page: Page,
	pane: Locator,
	sourceName: string,
	cell: number,
) {
	await pane.getByRole("button", { name: "Set Source", exact: true }).click();
	await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
	await page.locator(".dock-entry").filter({ hasText: "Cuelists" }).click();
	await page.locator(".cuelist-card").filter({ hasText: sourceName }).click();
	const restored = await activeVirtualPane(page);
	await restored
		.getByRole("button", {
			name: new RegExp(`Virtual playback page 1 cell ${cell} empty`),
		})
		.click();
	await expect(
		restored.getByRole("button", {
			name: new RegExp(
				`Virtual playback page 1 cell ${cell} ${escapeRegExp(sourceName)}`,
			),
		}),
	).toBeVisible();
}

export function selectTrigger(container: Locator, label: string): Locator {
	const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	return container
		.locator(".ui-form-field")
		.filter({ hasText: new RegExp(`^\\s*${escaped}`) })
		.locator(".ui-select-trigger");
}

export async function chooseSelect(
	page: Page,
	container: Locator,
	label: string,
	option: string,
) {
	await selectTrigger(container, label).click();
	const chooser = page.getByRole("dialog", {
		name: `Choose ${label} function`,
	});
	const escapedOption = option.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	await chooser
		.getByRole("button", { name: new RegExp(`^${escapedOption}(?:\\s|$)`) })
		.click();
}

export async function longPressPreload(page: Page) {
	const button = page.getByRole("button", { name: /^PRELOAD/ });
	await button.hover();
	await page.mouse.down();
	await page.waitForTimeout(750);
	await page.mouse.up();
	await expect(
		page.getByRole("button", { name: "PRELOAD", exact: true }),
	).toBeVisible();
}

function escapeRegExp(value: string) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
