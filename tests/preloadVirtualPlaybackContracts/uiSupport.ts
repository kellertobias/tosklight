import type { Locator, Page } from "@playwright/test";
import { expect } from "../bench/core/fixtures";

export async function openPlaybackMode(page: Page) {
	if (await page.locator(".playback-fader-bank").isVisible()) return;
	await page.locator(".mode-toggle").click();
	await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

export function playbackCard(page: Page, slotNumber: number): Locator {
	return page.locator(
		`.playback-fader-bank article[data-page="1"][data-playback-slot="${slotNumber}"]`,
	);
}

export async function addVirtualPlaybackPane(page: Page): Promise<Locator> {
	await showDockMode(page, "desks");
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
	// The Open Window catalog groups its cards into tabs, so the bench searches the tabs.
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText("Playbacks", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) break;
	}
	await card.first().click();
	const pane = page.locator(".desk-pane").filter({ hasText: "Playbacks" });
	await pane.getByRole("button", { name: "Create playback surface", exact: true }).click();
	return activeVirtualPane(page);
}

export async function activeVirtualPane(page: Page): Promise<Locator> {
	await showDockMode(page, "desks");
	const activeDesk = page.locator(".dock-list .dock-entry.active");
	if (!(await activeDesk.isVisible().catch(() => false)))
		await page
			.locator(".dock-list .dock-entry")
			.filter({ hasText: /Desk \d+/ })
			.last()
			.click();
	const pane = page
		.locator(".desk-pane")
		.filter({ has: page.locator(".playback-fader-bank") });
	await expect(pane).toBeVisible();
	return pane;
}

export async function assignVirtualSource(
	page: Page,
	pane: Locator,
	sourceName: string,
	cell: number,
) {
	await paneCard(pane,{page:1,position:400+cell}).click({button:"right"});
	const modal = page.getByRole("dialog", { name: "Playback Configuration" });
	await expect(modal).toBeVisible();
	await modal.getByLabel("Playback name").fill(sourceName);
	await modal
		.getByRole("radio", { name: sourceName, exact: true })
		.click();
	await modal.getByRole("button", { name: "Apply", exact: true }).click();
	await expect(modal).toBeHidden();
	await expect(paneCard(pane,{page:1,position:400+cell})).toContainText(sourceName);
}

async function showDockMode(
	page: Page,
	mode: "desks" | "builtins",
): Promise<void> {
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== mode)
		await toggle.click();
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

export function paneCard(pane:Locator,address:{page:number;position:number}):Locator {
 return pane.locator(`[data-page="${address.page}"][data-playback-slot="${address.position}"]`);
}
/**
 * Presses a pane cell's single action. A default pane shows each Playback as one square tile whose
 * button carries the action as its description; a pane with playback controls shows the button.
 */
export async function activatePaneCard(pane:Locator,address:{page:number;position:number}) {
 const card=paneCard(pane,address);
 const tile=card.locator(".playback-tile-action");
 const control=card.getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/});
 await expect(tile.or(control).first()).toBeVisible();
 if (await tile.count()) {
  await expect(tile).toHaveAccessibleDescription(/^(GO \+|TOGGLE|ON)( |$)/);
  await tile.click();
 } else await control.click();
}
export async function configurePaneGrid(page:Page,pane:Locator,rows:number,columns:number,pinnedPage?:number) {
 await pane.getByRole("button",{name:"Settings",exact:true}).click();
 const settings=page.getByRole("dialog",{name:"Pane Settings"});
 await settings.getByRole("tab",{name:"Playbacks",exact:true}).click();
 await settings.getByRole("button",{name:"Configure layout and page source"}).click();
 const layout=page.getByRole("dialog",{name:"Configure Playbacks"});
 while(await layout.locator("[data-playback-row-index]").count()>rows) await layout.getByRole("button",{name:/^Remove row /}).last().click();
 await layout.locator("[data-playback-row-index]").first().getByLabel("Playbacks in row").fill(String(columns));
 while(await layout.locator("[data-playback-row-index]").count()<rows) await layout.getByRole("button",{name:"Add Row",exact:true}).click();
 for(let index=0;index<rows;index++) {
 const row=layout.locator("[data-playback-row-index]").nth(index);
 await row.getByLabel("First Playback Number").fill(String(401+index*columns));
 await row.getByLabel("Playbacks in row").fill(String(columns));
 await row.getByLabel("Buttons",{exact:true}).fill("1");
 await chooseLayoutOption(page,row,"Input type","Buttons");
 }
 await chooseLayoutOption(page,layout,"Page source",pinnedPage===undefined?"Follow surface":"Pinned page");
 if(pinnedPage!==undefined) await layout.getByLabel("Page",{exact:true}).fill(String(pinnedPage));
 else await chooseLayoutOption(page,layout,"Follow surface","Main");
 await layout.getByRole("button",{name:"Apply",exact:true}).click();
 await expect(layout).toBeHidden();
 await settings.getByRole("button",{name:"Close settings"}).click();
}
async function chooseLayoutOption(page:Page,container:Locator,label:string,option:string) {
 await container.getByText(label,{exact:true}).locator("..").getByRole("button").click();
 await page.getByRole("listbox").getByRole("option",{name:option,exact:true}).click();
}
