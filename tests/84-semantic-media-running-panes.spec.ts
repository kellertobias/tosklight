import type { Page, Route } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

test.describe("docs/testing/14-media-and-running-panes.md", () => {
	test("MEDIA-001 @ui › Media is absent without an eligible patched server", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await expect(
			page
				.locator("[aria-label='Built-ins']")
				.getByRole("button", { name: "Media", exact: true }),
		).toHaveCount(0);
	});

	test("RUNNING-003 @ui › a Macro row filters and cancels through its exact live Off action", async ({
		api,
		desk,
		page,
	}) => {
		let active = true;
		let cancelRequest: { execution_id?: string } | null = null;
		await page.route("**/api/v2/macros/runtime", async (route) => {
			await fulfillJson(route, {
				desk_id: api.session?.desk.id ?? "desk",
				active: active ? [runningMacro()] : [],
				recent: active ? [] : [{ ...runningMacro(), state: "cancelled" }],
			});
		});
		await page.route("**/api/v2/macros/executions/cancel", async (route) => {
			cancelRequest = route.request().postDataJSON() as {
				execution_id?: string;
			};
			active = false;
			await fulfillJson(route, { ...runningMacro(), state: "cancelled" });
		});

		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await page
			.locator("[aria-label='Built-ins']")
			.getByRole("button", { name: "Running", exact: true })
			.click();

		const running = page.locator(".running-window");
		await expect(running).toBeVisible();
		await expect(running.getByText("9 · Reset desk", { exact: true })).toBeVisible();
		await expect(
			running.getByText("Macro · Cue — · Running", { exact: true }),
		).toBeVisible();

		await running.getByRole("button", { name: "Cuelists", exact: true }).click();
		await expect(running.getByText("No Cuelists are running.")).toBeVisible();
		await running.getByRole("button", { name: "Macros", exact: true }).click();

		await running
			.getByRole("button", {
				name: "Turn off Macro 9 Reset desk",
				exact: true,
			})
			.click();
		await expect.poll(() => cancelRequest).toEqual({
			execution_id: "execution-reset",
		});
		await expect(running.getByText("No Macros are running.")).toBeVisible();
		await expect(running.locator("[data-running-kind='macro']")).toHaveCount(0);
	});
});

async function openBuiltIns(page: Page): Promise<void> {
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== "builtins")
		await toggle.click();
}

async function fulfillJson(route: Route, body: unknown): Promise<void> {
	await route.fulfill({
		status: 200,
		contentType: "application/json",
		body: JSON.stringify(body),
	});
}

function runningMacro() {
	return {
		execution_id: "execution-reset",
		macro_id: "macro-reset",
		macro_number: 9,
		macro_name: "Reset desk",
		source_revision: 3,
		desk_id: "desk",
		user_id: "operator",
		session_id: "session",
		state: "running" as const,
		line: 1,
		command: "CLEAR",
		trigger: { type: "pool" as const },
		started_at: "2026-08-10T18:00:00Z",
	};
}
