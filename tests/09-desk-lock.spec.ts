import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

test.describe("docs/planned features/12-desk-lock.md", () => {
	test("LOCK-001 @ui @api › PIN lock covers every screen and blocks desk input without changing output", async ({ api, bench, desk, page }) => {
		await desk.open(bench.baseUrl);
		const otherDeskSession = api.session!;
		api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
		const before = await api.request<any>("GET", "/api/v1/dmx");
		await api.request("PUT", "/api/v1/desk-lock", { message: "Call the operator", wallpaper: null, unlock_mode: "pin", pin: "1234" });
		await api.request("POST", "/api/v1/desk-lock/lock", {});
		const lock = page.getByRole("dialog", { name: "Desk locked" });
		await expect(lock).toBeVisible();
		await expect(lock).toContainText("Call the operator");
		await expect(api.request("PUT", "/api/v1/master", { grand_master: 0.25 })).rejects.toThrow(/409.*desk is locked/);
		expect(await api.request<any>("GET", "/api/v1/dmx")).toEqual(before);
		api.session = otherDeskSession;
		expect((await api.request<any>("GET", "/api/v1/desk-lock")).locked).toBe(false);
		await api.request("PUT", "/api/v1/master", { grand_master: 1 });
		api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));

		await lock.getByLabel("PIN").fill("9999");
		await lock.getByRole("button", { name: "Unlock Desk" }).click();
		await expect(lock.getByText("Incorrect PIN")).toBeVisible();
		await expect(lock).toBeVisible();
		await lock.getByLabel("PIN").fill("1234");
		await lock.getByRole("button", { name: "Unlock Desk" }).click();
		await expect(lock).toBeHidden();
		await expect.poll(async () => (await api.request<any>("GET", "/api/v1/desk-lock")).locked).toBe(false);
	});
});
