import { expect, test } from "./bench/core/fixtures";
import type { ApiDriver } from "./bench/core/api";

interface ProgrammerProjection {
	session_id: string;
	command_line: string;
}

test.describe("docs/testing/10-desk-lock-and-operator-ui.md", () => {
	test("LOCK-001 @api @failure-mode › PIN lock covers every screen and control surface and drops every input without changing output", async ({ api, bench, desk, page }) => {
		await desk.open(bench.baseUrl);
		const pageDeskSession = await desk.session();
		const otherDeskSession = await api.request<typeof pageDeskSession>("POST", "/api/v2/sessions", {
			username: "Operator",
			client_id: crypto.randomUUID(),
		}, false);
		// Two clients are two windows of the one desk, not two desks.
		expect(otherDeskSession.desk.id).toBe(pageDeskSession.desk.id);
		api.session = pageDeskSession;
		const secondScreen = await page.context().newPage();
		await secondScreen.goto(bench.baseUrl);
		await expect(secondScreen.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
		await api.setCommandLineText("");
		const hardware = await bench.osc();
		await hardware.subscribe(`desk-lock-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
		const before = await api.request<any>("GET", "/api/v2/output/dmx");
		const wallpaper = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='8'%3E%3Cpath fill='%23123456' d='M0 0h8v8H0z'/%3E%3C/svg%3E";
		try {
			await api.request("PUT", "/api/v2/desk-lock", { message: "Call the operator", wallpaper, unlock_mode: "pin", pin: "1234" });
			await api.request("POST", "/api/v2/desk-lock/lock", {});
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

			await expect(api.request("POST", "/api/v2/output-runtime/global-master/actions", { grand_master: 0.25 })).rejects.toThrow(/409.*desk is locked/);
			await expect(api.executeCommandLine("1 AT 50")).rejects.toThrow(/desk is locked/i);
			await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-5`, [true]);
			await page.waitForTimeout(100);
			expect(await commandLine(api)).toBe("");
			expect(await api.request<any>("GET", "/api/v2/output/dmx")).toEqual(before);

			// Desk Lock is installation-wide: a session that logged in on another desk record is
			// locked with the rest, and cannot move the Grand Master past it.
			api.session = otherDeskSession;
			expect((await api.request<any>("GET", "/api/v2/desk-lock")).locked).toBe(true);
			await expect(
				api.request("POST", "/api/v2/output-runtime/global-master/actions", { grand_master: 1 }),
			).rejects.toThrow(/409.*desk is locked/);
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
			await expect.poll(async () => (await api.request<any>("GET", "/api/v2/desk-lock")).locked).toBe(false);

			await page.waitForTimeout(100);
			expect(await commandLine(api)).toBe("");
			await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-1`, [true]);
			await expect.poll(() => commandLine(api)).toBe("F1");
			await expect(page.getByRole("textbox", { name: "Command line", exact: true })).toHaveValue("F1");
			await lateScreen.close();
		} finally {
			await hardware.close();
			await secondScreen.close();
		}
	});

});

async function commandLine(api: ApiDriver): Promise<string> {
	const programmers = await api.request<ProgrammerProjection[]>("GET", "/api/v2/programmers");
	return programmers.find((programmer) => programmer.session_id === api.session?.session_id)?.command_line ?? "";
}
