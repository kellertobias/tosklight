import { expect, test } from "./bench/core/fixtures";

test("ERROR-COPY @ui › connection errors copy complete server diagnostics with an icon-only button", async ({ page, context, api }) => {
	await context.grantPermissions(["clipboard-read", "clipboard-write"]);
	const diagnostic = "Bootstrap failed\nrequest_id: desk-copy-128\ntrace:\n  engine::restore\n  server::bootstrap";
	await page.route("**/api/v2/bootstrap", (route) => route.fulfill({ status: 500, contentType: "text/plain", body: diagnostic }));
	await page.goto(api.baseUrl);
	const alert = page.getByRole("alert").filter({ hasText: "desk-copy-128" }).first();
	await expect(alert).toBeVisible();
	const button = alert.getByRole("button", { name: "Copy error", exact: true });
	await expect(button).toBeVisible();
	await expect(button).toHaveText("");
	await button.click();
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain(diagnostic);
	await expect(alert).toBeVisible();
});

test("ERROR-COPY @ui › a rejected command has copy controls in software and attached-hardware history", async ({ page, context, api, desk, bench }) => {
	await context.grantPermissions(["clipboard-read", "clipboard-write"]);
	await desk.open(api.baseUrl);
	const input = page.getByRole("textbox", { name: "Command line", exact: true });
	await input.fill("FIXTURE 1 AT 101");
	await input.press("Enter");
	await input.click();
	const history = page.getByRole("dialog", { name: "Command line history", exact: true });
	await expect(history).toBeVisible();
	const rejected = history.locator(".command-history-entry.rejected").first();
	await expect(rejected).toContainText(/within 0-100/i);
	const feedback = await rejected.locator("p").innerText();
	const copy = rejected.getByRole("button", { name: "Copy error", exact: true });
	await expect(copy).toHaveText("");
	await copy.click();
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`FIXTURE 1 AT 101\n${feedback}`);
	const hardware = await bench.osc();
	const clientId = `error-copy-${crypto.randomUUID()}`;
	try {
		await hardware.subscribe(clientId, "desk");
		await expect.poll(async () => (await api.request<{ hardware_connected: boolean }>("GET", "/api/v2/bootstrap", undefined, false)).hardware_connected).toBe(true);
		await expect(page.locator(".command-line-bar.hardware-mode")).toBeVisible();
		await history.getByRole("button", { name: "Close command line history" }).click();
		await input.click();
		await expect(copy).toBeVisible();
		await copy.click();
		await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`FIXTURE 1 AT 101\n${feedback}`);
	} finally {
		await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
		await hardware.close();
	}
});
