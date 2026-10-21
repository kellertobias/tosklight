import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
async function configured(page: Page, subject: string) {
	await page.setViewportSize({ width: 1200, height: 860 });
	await page.goto(
		`/iframe.html?id=tosklight-windows-configured-setup--${subject}&viewMode=story`,
	);
	await expect(page.locator("[data-documentation-shot]")).toHaveAttribute(
		"data-documentation-ready",
		"true",
	);
}
test("Setup documentation shows assigned sound input and an installed native extension", async ({
	page,
}) => {
	await configured(page, "sound-input");
	await expect(page.locator(".sound-input-settings")).toContainText(
		"Microphone permission: granted",
	);
	await expect(
		page.getByRole("button", {
			name: "USB Audio Interface · Line Input 1–2",
			exact: true,
		}),
	).toBeVisible();
	await configured(page, "native-extensions");
	await expect(page.locator(".native-extension-status")).toContainText(
		"1 package · 1 running instance",
	);
	await page.locator(".native-extension-status summary").click();
	await expect(page.locator(".native-extension-package")).toContainText(
		"ToskLight Hardware Simulator",
	);
	await expect(page.locator(".native-extension-instance")).toContainText(
		"hardware-simulator",
	);
});
test("Setup documentation shows Matter pairing, populated network routes, and a USB route editor", async ({
	page,
}) => {
	await configured(page, "matter-bridge");
	await expect(page.locator(".matter-desk-settings")).toContainText(
		"2 assigned playbacks exposed: 1 dimmable, 1 color",
	);
	await expect(page.getByText("34970112332", { exact: true })).toBeVisible();
	await configured(page, "dmx-routes");
	await expect(page.locator(".output-routes-setup article")).toHaveCount(4);
	await expect(page.locator(".output-routes-setup")).toContainText("Art-Net");
	await expect(page.locator(".output-routes-setup")).toContainText("sACN");
	await configured(page, "usb-dmx");
	await expect(page.locator(".output-routes-setup")).toContainText(
		"DMX USB PRO · USB serial DEMO001",
	);
	await page.getByRole("button", { name: "Edit route", exact: true }).click();
	await expect(
		page.getByRole("dialog", { name: "Output route editor" }),
	).toBeVisible();
	await expect(
		page.getByLabel("Logical universe", { exact: true }),
	).toHaveValue("1");
});
test("Full desk documentation shows one readable eight-playback bank", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1600, height: 920 });
	await page.goto(
		"/iframe.html?id=tosklight-marketing--full-application&viewMode=story",
	);
	const cards = page.locator(
		".control-section.playbacks article[data-playback-slot]",
	);
	await expect(cards).toHaveCount(8);
	await expect(cards.first()).toBeVisible();
	await expect(cards.last()).toBeVisible();
	await expect(
		page.locator(".control-section.playbacks .playback-fader-bank"),
	).toHaveAttribute("data-playback-rows", "1");
});
