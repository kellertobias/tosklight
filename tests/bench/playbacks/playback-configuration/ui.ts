import { expect, type Locator, type Page } from "@playwright/test";

export async function openPlaybackMode(page: Page): Promise<void> {
	if (await page.locator(".playback-fader-bank").isVisible()) return;
	await page.locator(".mode-toggle").click();
	await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

export async function armSet(page: Page): Promise<void> {
	await page.getByRole("button", { name: "SET", exact: true }).click();
}

export function playbackCard(page: Page, slot: number): Locator {
	return page.locator(
		`.playback-fader-bank article[data-playback-slot="${slot}"]`,
	);
}

export function playbackSlider(page: Page, slot: number): Locator {
	return playbackCard(page, slot).getByRole("slider");
}

export async function expectConfigurationModal(
	page: Page,
	playbackPage: number,
	slot: number,
): Promise<Locator> {
	const modal = page.getByRole("dialog", { name: "Playback Configuration" });
	await expect(modal).toHaveCount(1);
	await expect(modal).toBeVisible();
	await expect(modal).toHaveAttribute("data-page", String(playbackPage));
	await expect(modal).toHaveAttribute("data-slot", String(slot));
	return modal;
}

/**
 * One option in the configuration modal's Options group, by the name the operator reads.
 *
 * The function radios — Cue List, Group Master, Special — are named outright, but an option
 * radio carries its identity in the card it wraps: a number beside a name. Matching that name is
 * what an operator picking "Cuelist 2" out of the list actually does.
 */
export function optionRadio(
	container: Locator,
	page: Page,
	label: string,
): Locator {
	return container
		.getByRole("radio")
		.filter({ has: page.getByText(label, { exact: true }) });
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
): Promise<void> {
	const trigger = selectTrigger(container, label);
	await trigger.click();
	if ((await trigger.getAttribute("aria-haspopup")) === "dialog") {
		const dialog = page.getByRole("dialog", {
			name: `Choose ${label} function`,
		});
		await dialog
			.getByRole("button")
			.filter({
				hasText: new RegExp(
					`^${option.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`,
				),
			})
			.click();
		return;
	}
	await page.getByRole("option", { name: option, exact: true }).click();
}

export async function choosePlaybackColor(
	page: Page,
	container: Locator,
	color: string,
): Promise<void> {
	const before = await container.boundingBox();
	await container
		.locator(".ui-form-field", { hasText: "Playback color" })
		.locator(".ui-color-input-trigger")
		.click();
	await expect(
		page.locator(".ui-color-dropdown-backdrop .ui-color-dropdown-panel"),
	).toBeVisible();
	const after = await container.boundingBox();
	expect(after?.width).toBeCloseTo(before?.width ?? 0, 0);
	expect(after?.height).toBeCloseTo(before?.height ?? 0, 0);
	await page
		.getByRole("option", { name: `Use color ${color}`, exact: true })
		.click();
}

export async function addVirtualPlaybackPane(page: Page): Promise<Locator> {
	await page
		.getByRole("button", { name: "Desktops / Built-ins", exact: true })
		.click();
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
		.filter({ has: page.getByText("Virtual Playbacks", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) break;
	}
	await card.first().click();
	const pane = page
		.locator(".desk-pane")
		.filter({ hasText: "Virtual Playbacks" });
	await expect(pane).toBeVisible();
	return pane;
}
