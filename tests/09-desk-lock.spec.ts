import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";

interface ProgrammerProjection {
	session_id: string;
	command_line: string;
}

interface DeskSessionProjection {
	desk: { osc_alias: string };
}

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
			await expect(api.executeCommandLine("1 AT 50")).rejects.toThrow(/desk is locked/i);
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
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Lock Desk", exact: true }).click();
		const lock = page.getByRole("dialog", { name: "Desk locked" });
		await expect(lock).toBeVisible();
		await expect(lock).toContainText("This desk is locked.");
		await expect(lock.getByLabel("PIN")).toHaveCount(0);
		await lock.getByRole("button", { name: "Unlock Desk" }).click();
		await expect(lock).toBeHidden();
	});

	test("LOCK-001 @ui › Screens owns shortcut configuration and the Desk Lock settings modal", async ({ api, bench, desk, page }) => {
		await desk.open(bench.baseUrl);
		api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
		await page.locator(".setup-window nav").getByRole("button", { name: "Screens & playback", exact: true }).click();

		const defaultScreen = page.locator(".default-screen-settings");
		const undo = page.getByRole("button", { name: "Undo", exact: true });
		await expect(undo).toBeDisabled();
		await expect(page.getByRole("button", { name: "Save changes", exact: true })).toHaveCount(0);

		await defaultScreen.getByRole("button", { name: "Configure Playbacks", exact: true }).click();
		await page.getByRole("button", { name: "Close playback configuration" }).click();
		await expect(undo).toBeDisabled();
		await page.locator(".setup-window nav").getByRole("button", { name: "Network & Inputs", exact: true }).click();
		await page.locator(".setup-window nav").getByRole("button", { name: "Screens & playback", exact: true }).click();
		await expect(undo).toBeDisabled();

		const originalAlias = await defaultScreen.getByLabel("OSC alias").inputValue();
		const changedAlias = `${originalAlias}-undo`;
		await defaultScreen.getByLabel("OSC alias").fill(changedAlias);
		await expect(undo).toBeEnabled();
		await expect.poll(() => currentDeskAlias(api)).toBe(changedAlias);

		await page.locator(".setup-window nav").getByRole("button", { name: "Network & Inputs", exact: true }).click();
		await page.locator(".setup-window nav").getByRole("button", { name: "Screens & playback", exact: true }).click();
		await expect(undo).toBeEnabled();
		await undo.click();
		await expect(defaultScreen.getByLabel("OSC alias")).toHaveValue(originalAlias);
		await expect(undo).toBeDisabled();
		await expect.poll(() => currentDeskAlias(api)).toBe(originalAlias);

		const shortcuts = defaultScreen.getByRole("switch", { name: "Enable software keyboard shortcuts" });
		await expect(shortcuts).toBeChecked();
		await shortcuts.locator("..").locator(".ui-switch-track").click();
		await expect(shortcuts).not.toBeChecked();
		await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("light.desk-controls") ?? "{}").regularNumberShortcuts)).toBe(false);

		await page.getByRole("button", { name: "Desk Lock", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Desk Lock" });
		const titleBar = settings.locator(":scope > .ui-modal-titlebar");
		await expect(titleBar.getByRole("button", { name: "Save Lock Configuration" })).toBeVisible();
		await settings.getByLabel("Lock message").fill("Stand by for the operator");
		await titleBar.getByRole("button", { name: "Save Lock Configuration" }).click();
		await expect(settings).toBeHidden();
		await expect.poll(async () => (await api.request<any>("GET", "/api/v1/desk-lock")).message).toBe("Stand by for the operator");
	});
});

async function commandLine(api: ApiDriver): Promise<string> {
	const programmers = await api.request<ProgrammerProjection[]>("GET", "/api/v1/programmers");
	return programmers.find((programmer) => programmer.session_id === api.session?.session_id)?.command_line ?? "";
}

async function currentDeskAlias(api: ApiDriver): Promise<string> {
	const deskId = api.session?.desk.id;
	if (!deskId) throw new Error("Expected an authenticated desk session");
	const session = await api.request<DeskSessionProjection>("POST", "/api/v1/sessions", {
		username: "Operator",
		desk_id: deskId,
	}, false);
	return session.desk.osc_alias;
}
