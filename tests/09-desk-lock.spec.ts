import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

test.describe("docs/testing/10-desk-lock-and-operator-ui.md", () => {
	test("LOCK-001 @ui @api @osc › PIN lock covers every screen and drops every desk input without changing output", async ({ api, bench, desk, page }) => {
		await desk.open(bench.baseUrl);
		const pageDeskSession = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
		const otherDeskSession = await api.request<typeof pageDeskSession>("POST", "/api/v1/sessions", {
			username: "Operator",
			client_id: crypto.randomUUID(),
		}, false);
		expect(otherDeskSession.desk.id).not.toBe(pageDeskSession.desk.id);
		api.session = pageDeskSession;
		const secondScreen = await page.context().newPage();
		await secondScreen.goto(bench.baseUrl);
		await expect(secondScreen.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
		await api.command("programmer.command_line", { value: "" });
		const hardware = await bench.osc();
		await hardware.subscribe(`desk-lock-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
		const before = await api.request<any>("GET", "/api/v1/dmx");
		const wallpaper = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='8'%3E%3Cpath fill='%23123456' d='M0 0h8v8H0z'/%3E%3C/svg%3E";
		try {
			await api.request("PUT", "/api/v1/desk-lock", { message: "Call the operator", wallpaper, unlock_mode: "pin", pin: "1234" });
			await api.request("POST", "/api/v1/desk-lock/lock", {});
			const lock = page.getByRole("dialog", { name: "Desk locked" });
			const secondLock = secondScreen.getByRole("dialog", { name: "Desk locked" });
			await expect(lock).toBeVisible();
			await expect(secondLock).toBeVisible();
			await expect(lock).toContainText("Call the operator");
			await expect(secondLock).toContainText("Call the operator");
			await expect(lock).toHaveCSS("background-image", /data:image\/svg\+xml/);

			const lateScreen = await page.context().newPage();
			await lateScreen.goto(bench.baseUrl);
			const lateLock = lateScreen.getByRole("dialog", { name: "Desk locked" });
			await expect(lateLock).toBeVisible();
			await expect(lateLock).toContainText("Call the operator");

			await expect(api.request("PUT", "/api/v1/master", { grand_master: 0.25 })).rejects.toThrow(/409.*desk is locked/);
			await expect(api.command("programmer.execute", { value: "1 AT 50" })).rejects.toThrow(/desk is locked/i);
			await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-5`, [true]);
			await page.waitForTimeout(100);
			expect(await commandLine(api)).toBe("");
			expect(await api.request<any>("GET", "/api/v1/dmx")).toEqual(before);

			api.session = otherDeskSession;
			expect((await api.request<any>("GET", "/api/v1/desk-lock")).locked).toBe(false);
			await api.request("PUT", "/api/v1/master", { grand_master: 1 });
			api.session = pageDeskSession;

			await lock.getByLabel("PIN").fill("9999");
			await lock.getByRole("button", { name: "Unlock Desk" }).click();
			await expect(lock.getByText("Incorrect PIN")).toBeVisible();
			await expect(lock).toBeVisible();
			await expect(secondLock).toBeVisible();
			await lock.getByLabel("PIN").fill("1234");
			await lock.getByRole("button", { name: "Unlock Desk" }).click();
			await expect(lock).toBeHidden();
			await expect(secondLock).toBeHidden();
			await expect(lateLock).toBeHidden();
			await expect.poll(async () => (await api.request<any>("GET", "/api/v1/desk-lock")).locked).toBe(false);

			await page.waitForTimeout(100);
			expect(await commandLine(api)).toBe("");
			await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-1`, [true]);
			await expect.poll(() => commandLine(api)).toBe("F1");
			await expect(page.getByLabel("Command line")).toHaveValue("F1");
			await lateScreen.close();
		} finally {
			await hardware.close();
			await secondScreen.close();
		}
	});

	test("LOCK-001 @ui › button mode has no PIN and uses the readable fallback lock screen", async ({ api, bench, desk, page }) => {
		await desk.open(bench.baseUrl);
		api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
		await api.request("PUT", "/api/v1/desk-lock", {
			message: "",
			wallpaper: "https://invalid.example.test/unavailable-lock-screen.png",
			unlock_mode: "button",
			pin: null,
		});
		await api.request("POST", "/api/v1/desk-lock/lock", {});
		const lock = page.getByRole("dialog", { name: "Desk locked" });
		await expect(lock).toBeVisible();
		await expect(lock).toContainText("This desk is locked.");
		await expect(lock.getByLabel("PIN")).toHaveCount(0);
		await lock.getByRole("button", { name: "Unlock Desk" }).click();
		await expect(lock).toBeHidden();
	});
});

async function commandLine(api: any): Promise<string> {
	const programmers = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
	return programmers.find((programmer) => programmer.session_id === api.session?.session_id)?.command_line ?? "";
}
