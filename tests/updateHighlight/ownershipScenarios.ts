import { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy, programmer } from "../support/catalog";
import {
	assertReachableAlert,
	fixturesByNumber,
	hardwareHighlightGeometry,
	highlightAction,
	highlightKey,
	highlightState,
	openBuiltIn,
	softwareHighlightGeometry,
} from "../support/updateHighlight/highlight";

test("HIGHLIGHT-004 @api › ownership conflicts retain same-user sessions, release on the last session, and stay desk-local", async ({
	api,
	bench,
}) => {
	await loadCanonicalCopy(api, bench, "highlight-004", "default-stage");
	const fixtures = await fixturesByNumber(api, [101, 102, 103]);
	await api.request("POST", "/api/v1/users", {
		name: "Highlight A",
		enabled: true,
	});
	await api.request("POST", "/api/v1/users", {
		name: "Highlight B",
		enabled: true,
	});

	const userAFirst = new ApiDriver(api.baseUrl);
	userAFirst.session = await userAFirst.request(
		"POST",
		"/api/v1/sessions",
		{ username: "Highlight A", desk_id: api.session!.desk.id },
		false,
	);
	const userASecond = new ApiDriver(api.baseUrl);
	userASecond.session = await userASecond.request(
		"POST",
		"/api/v1/sessions",
		{ username: "Highlight A", desk_id: api.session!.desk.id },
		false,
	);
	const userB = new ApiDriver(api.baseUrl);
	userB.session = await userB.request(
		"POST",
		"/api/v1/sessions",
		{ username: "Highlight B", desk_id: api.session!.desk.id },
		false,
	);

	await userAFirst.command("selection.set", { fixtures: [fixtures[0].id] });
	await userB.command("selection.set", { fixtures: [fixtures[1].id] });
	await highlightAction(userAFirst, "on");
	const ownerBeforeConflict = await highlightState(userAFirst);
	expect(ownerBeforeConflict).toMatchObject({
		active: true,
		output_enabled: true,
		owner_user_name: "Highlight A",
	});
	expect(ownerBeforeConflict.remembered).toHaveLength(1);
	await expect(highlightAction(userB, "toggle")).rejects.toThrow(
		/another user on this desk/i,
	);
	expect(await highlightState(userAFirst)).toMatchObject(ownerBeforeConflict);
	expect(await highlightState(userB)).toMatchObject({
		active: false,
		output_enabled: false,
	});

	await highlightAction(userB, "next");
	expect((await programmer(userB)).selected).toEqual([fixtures[1].id]);
	expect(await highlightState(userAFirst)).toMatchObject({
		active: true,
		output_enabled: true,
		owner_user_name: "Highlight A",
	});
	await userASecond.request(
		"DELETE",
		`/api/v1/sessions/${userASecond.session!.session_id}`,
	);
	expect((await highlightState(userAFirst)).active).toBe(true);
	await expect(highlightAction(userB, "on")).rejects.toThrow(
		/another user on this desk/i,
	);

	await userAFirst.request(
		"DELETE",
		`/api/v1/sessions/${userAFirst.session!.session_id}`,
	);
	await highlightAction(userB, "on");
	expect(await highlightState(userB)).toMatchObject({
		active: true,
		output_enabled: true,
		owner_user_name: "Highlight B",
	});

	const otherDesk = new ApiDriver(api.baseUrl);
	await otherDesk.login("Highlight A");
	await otherDesk.command("selection.set", { fixtures: [fixtures[2].id] });
	await highlightAction(otherDesk, "on");
	expect(await highlightState(otherDesk)).toMatchObject({
		active: true,
		output_enabled: true,
		owner_user_name: "Highlight A",
		remembered: [{ fixture_id: fixtures[2].id }],
	});
	expect((await highlightState(userB)).owner_user_name).toBe("Highlight B");
});

test("HIGHLIGHT-005 @ui › Highlight errors remain reachable above production content without moving accepted controls", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await loadCanonicalCopy(api, bench, "highlight-005", "default-stage");
	const errors = [
		{
			status: 409,
			message: "Highlight output is active for another user on this desk",
		},
		{ status: 500, message: "The Highlight action was rejected by the desk" },
	];

	for (const viewport of [
		{ width: 1280, height: 720 },
		{ width: 1600, height: 1100 },
	]) {
		await page.setViewportSize(viewport);
		await desk.open(bench.baseUrl);
		await openBuiltIn(page, "Fixtures");
		await expect(page.locator(".programmer-number-block")).toBeVisible();

		for (const error of errors) {
			const before = await softwareHighlightGeometry(page);
			await page.route(
				"**/api/v1/highlight/action",
				async (route) => {
					await route.fulfill({
						status: error.status,
						contentType: "application/json",
						body: JSON.stringify({ error: error.message }),
					});
				},
				{ times: 1 },
			);
			await highlightKey(page, "HIGH").click();
			const alert = page.locator("[data-highlight-error-alert]");
			await expect(alert).toHaveCount(1);
			await expect(alert).toContainText(error.message);
			await page.getByRole("button", { name: /Open show menu/ }).click();
			const modal = page.getByRole("dialog", { name: "Show", exact: true });
			await expect(modal).toBeVisible();
			await assertReachableAlert(page, alert, modal, viewport);
			expect(await softwareHighlightGeometry(page)).toEqual(before);
			await expect(highlightKey(page, "HIGH")).toHaveText("HIGH");
			await expect(
				page.locator(".command-line-bar [aria-label='Highlight status']"),
			).toHaveCount(0);
			const dismiss = page.getByRole("button", {
				name: "Dismiss Highlight error",
			});
			await dismiss.focus();
			await expect(dismiss).toBeFocused();
			await dismiss.press("Enter");
			await expect(alert).toBeHidden();
			await page.getByRole("button", { name: "Close Show" }).click();
			await expect(modal).toBeHidden();
		}

		const hardware = await bench.osc();
		const clientId = `highlight-005-${viewport.width}-${crypto.randomUUID()}`;
		try {
			await page.route(
				"**/api/v1/highlight/action",
				async (route) => {
					await route.fulfill({
						status: 409,
						contentType: "application/json",
						body: JSON.stringify({ error: errors[0].message }),
					});
				},
				{ times: 1 },
			);
			await highlightKey(page, "HIGH").click();
			const alert = page.locator("[data-highlight-error-alert]");
			await expect(alert).toBeVisible();
			await hardware.subscribe(clientId, api.session!.desk.osc_alias);
			await expect
				.poll(
					async () =>
						(
							await api.request<any>(
								"GET",
								"/api/v1/bootstrap",
								undefined,
								false,
							)
						).hardware_connected,
				)
				.toBe(true);
			await expect(
				page.locator(".hardware-right-pane .hardware-control-summary"),
			).toBeVisible();
			const hardwareBefore = await hardwareHighlightGeometry(page);
			await page.getByRole("button", { name: /Open show menu/ }).click();
			const modal = page.getByRole("dialog", { name: "Show", exact: true });
			await assertReachableAlert(page, alert, modal, viewport);
			expect(await hardwareHighlightGeometry(page)).toEqual(hardwareBefore);
			await page
				.getByRole("button", { name: "Dismiss Highlight error" })
				.click();
			await page.getByRole("button", { name: "Close Show" }).click();
		} finally {
			await hardware
				.send("/light/unsubscribe", [clientId])
				.catch(() => undefined);
			await hardware.close();
			await expect
				.poll(
					async () =>
						(
							await api.request<any>(
								"GET",
								"/api/v1/bootstrap",
								undefined,
								false,
							)
						).hardware_connected,
				)
				.toBe(false);
		}
	}
});
