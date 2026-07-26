import { expect, test } from "./bench/core/fixtures";

test("PATCH-PERF-001 @ui › warmed Patch actions retain informational action-to-visible p50 and p95", async ({
	bench,
	desk,
	page,
}, testInfo) => {
	test.setTimeout(120_000);
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	const mib = page.getByRole("button", {
		name: "Move in Black 1",
		exact: true,
	});
	await expect(mib).toBeVisible();

	const patchRequests: string[] = [];
	const unrelatedReads: string[] = [];
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (request.method() === "POST" && url.pathname === "/api/v2/patch/fixtures") {
			patchRequests.push(url.pathname);
		} else if (
			request.method() === "GET" &&
			[
				"/api/v2/fixtures",
				"/api/v2/configuration",
				"/api/v2/shows",
				"/api/v2/media",
			].some((path) => url.pathname.startsWith(path))
		) {
			unrelatedReads.push(url.pathname);
		}
	});

	const samples = 30;
	for (let index = 0; index < samples; index++) {
		const enabled = index % 2 !== 0;
		await page.getByRole("button", { name: "SET", exact: true }).click();
		await mib.click();
		const editor = page.locator(".patch-edit-modal");
		await editor
			.getByLabel("Move in Black value")
			.selectOption(enabled ? "true" : "false");
		await editor.getByRole("button", { name: "Set", exact: true }).click();
		await expect(mib).toHaveText(enabled ? "On" : "Off");
		await expect
			.poll(async () => {
				const diagnostics =
					await page.evaluate(() =>
						window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot(),
					);
				return diagnostics?.patchActionToVisible.samples ?? 0;
			})
			.toBe(index + 1);
	}

	const diagnostics = await page.evaluate(() =>
		window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot(),
	);
	expect(diagnostics?.patchActionToVisible).toMatchObject({
		samples,
		p50Ms: expect.any(Number),
		p95Ms: expect.any(Number),
		gateEnforced: false,
	});
	expect(patchRequests).toHaveLength(samples);
	expect(unrelatedReads).toEqual([]);
	await testInfo.attach("patch-action-to-visible.json", {
		body: JSON.stringify(
			{
				action_to_visible: diagnostics?.patchActionToVisible,
				patch_request_count: patchRequests.length,
				unrelated_reads: unrelatedReads,
			},
			null,
			2,
		),
		contentType: "application/json",
	});
});
