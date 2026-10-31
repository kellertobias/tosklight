import { expect, test } from "./bench/core/fixtures";

test("@ui Fixture Sheet touch inspection reports the retained Programmer source", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.open(bench.baseUrl);
	const entry = page
		.locator(".dock-entry")
		.filter({ hasText: "Fixtures" })
		.first();
	if (!(await entry.isVisible()))
		await page
			.getByRole("button", { name: "Desktops / Built-ins", exact: true })
			.click();
	await entry.click();
	await page
		.getByRole("button", { name: "Value sources", exact: true })
		.click();
	const dialog = page.getByRole("dialog", { name: "Fixture value sources" });
	await expect(dialog).toBeVisible();
	await api.executeCommandLine("FIXTURE 1 AT 50");
	await dialog.getByRole("button", { name: "Refresh sources" }).click();
	await expect(
		dialog.getByRole("cell", { name: "Programmer", exact: true }),
	).toBeVisible();
	await expect(
		dialog.getByRole("cell", { name: "50%", exact: true }),
	).toBeVisible();
	await page.setViewportSize({ width: 1024, height: 768 });
	await expect(
		dialog.getByRole("button", { name: "Inspect fixture" }),
	).toBeVisible();
	await expect(
		dialog.getByRole("button", { name: "Refresh sources" }),
	).toBeInViewport();
});
