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
	const mib = page.getByRole("button", { name: /^MIB 1:/ });
	await expect(mib).toBeVisible();

	const patchRequests: Array<{
		path: string;
		requestId: string | undefined;
		action: string | undefined;
	}> = [];
	const unrelatedReads: string[] = [];
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (
			request.method() === "POST" &&
			/^\/api\/v2\/patch\/fixtures\/[^/]+\/update$/.test(url.pathname)
		) {
			const body = request.postDataJSON() as {
				request_id?: string;
				action?: string;
			};
			patchRequests.push({
				path: url.pathname,
				requestId: body.request_id,
				action: body.action,
			});
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
			.getByLabel("MIB value: Off or non-negative seconds")
			.fill(enabled ? "0" : "Off");
		await editor.getByRole("button", { name: "Set", exact: true }).click();
		await expect(mib).toHaveText(enabled ? "0 s" : "Off");
		await expect
			.poll(async () => {
				const diagnostics = await page.evaluate(() =>
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
	expect(new Set(patchRequests.map(({ requestId }) => requestId)).size).toBe(
		samples,
	);
	expect(new Set(patchRequests.map(({ action }) => action))).toEqual(
		new Set(["set_move_in_black"]),
	);
	expect(unrelatedReads).toEqual([]);
	await testInfo.attach("patch-action-to-visible.json", {
		body: JSON.stringify(
			{
				action_to_visible: diagnostics?.patchActionToVisible,
				logical_patch_request_count: new Set(
					patchRequests.map(({ requestId }) => requestId),
				).size,
				raw_patch_request_count: patchRequests.length,
				patch_requests: patchRequests,
				unrelated_reads: unrelatedReads,
			},
			null,
			2,
		),
		contentType: "application/json",
	});
});
