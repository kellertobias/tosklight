import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";
import { installPlannedDemoDynamics } from "./support/plannedDemoDynamics";

test("DYNAMICS-POOL-001 @ui › stored Dynamic 29 stays painted through navigation and reconnect", async ({
	api,
	bench,
	desk,
	page,
}) => {
	test.setTimeout(90_000);
	const show = await loadCanonicalCopy(
		api,
		bench,
		"dynamic-29-visibility",
		"default-stage",
	);
	for (const groupId of ["4", "5", "8", "11", "12", "18", "19", "27"]) {
		if ((await api.showObject(show.id, "group", groupId)) != null) continue;
		await api.seedShowObject(show.id, "group", groupId, {
			id: groupId,
			name: `Dynamic test group ${groupId}`,
			fixtures: [],
			color: null,
			icon: "◇",
			derived_from: null,
			frozen_from: null,
			programming: {},
		});
	}
	const definitions = await installPlannedDemoDynamics(api, show.id, {
		assignVirtualPlaybacks: false,
	});
	const stored29 = definitions.find((dynamic) => dynamic.pool_number === 29);
	expect(stored29).toBeDefined();

	await desk.open(api.baseUrl);
	await openDynamics(page);
	await assertStoredDynamic29(page, stored29.id);

	const tile = page.locator('[data-pool-position="28"]');
	await tile.scrollIntoViewIfNeeded();
	await tile.click({ button: "right" });
	await expect(page.locator(".dynamics-editor")).toBeVisible();
	await page.getByRole("button", { name: "← Dynamics" }).click();
	await assertStoredDynamic29(page, stored29.id);

	// Reload is the real browser/session reconnect path. The collection snapshot may arrive in a
	// different storage-ID order, but pool address 29 must retain the same authoritative object.
	await page.reload();
	await openDynamics(page);
	await assertStoredDynamic29(page, stored29.id);
});

async function openDynamics(page: import("@playwright/test").Page) {
	const dockMode = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await dockMode.getAttribute("data-dock-mode")) !== "builtins")
		await dockMode.click();
	await page.locator(".dock-entry").filter({ hasText: "Dynamics" }).click();
	await expect(page.locator(".dynamics-window")).toBeVisible();
}

async function assertStoredDynamic29(
	page: import("@playwright/test").Page,
	storageId: string,
) {
	const tile = page.locator('[data-pool-position="28"]');
	await expect(tile).toHaveAttribute("data-pool-slot-id", storageId);
	await expect(tile).not.toHaveClass(/empty/);
	await expect(tile).toContainText("Sunstrip Rain");
	await expect(tile).toBeVisible();
	expect(
		await tile.evaluate((node) => getComputedStyle(node).contentVisibility),
	).toBe("visible");
}
