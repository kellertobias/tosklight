import { ApiDriver } from "../bench/core/api";
import { expect, test } from "../bench/core/fixtures";
import { replaceProgrammingSelection } from "../bench/command-selection/programmingSelection";
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

test("HIGHLIGHT-004 @api › Highlight belongs to the desk and every surface shares it", async ({
	api,
	bench,
}) => {
	const show = await loadCanonicalCopy(
		api,
		bench,
		"highlight-004",
		"default-stage",
	);
	const fixtures = await fixturesByNumber(api, [101, 102, 103]);

	// Three connections to the one desk: a main window, a second screen, and a wing.
	const mainWindow = new ApiDriver(api.baseUrl);
	const secondScreen = new ApiDriver(api.baseUrl);
	const wing = new ApiDriver(api.baseUrl);
	for (const surface of [mainWindow, secondScreen, wing]) {
		await surface.login("Operator");
	}

	await replaceProgrammingSelection(mainWindow, {
		surface: "api",
		showId: show.id,
		fixtures: [fixtures[0].id],
	});
	await highlightAction(mainWindow, "on");
	const held = await highlightState(mainWindow);
	expect(held).toMatchObject({ active: true, output_enabled: true });
	expect(held.remembered).toHaveLength(1);

	// There is no second operator to be refused: every surface reads the same Highlight, and none
	// of them is told who owns it, because nobody does.
	for (const surface of [secondScreen, wing]) {
		const shared = await highlightState(surface);
		expect(shared).toMatchObject({ active: true, output_enabled: true });
		expect(shared).not.toHaveProperty("owner_user_id");
		expect(shared).not.toHaveProperty("owner_user_name");
	}

	// And every surface may step it, because it is the desk's Highlight and the desk's selection.
	await highlightAction(wing, "next");
	expect((await programmer(mainWindow)).selected).toEqual([fixtures[0].id]);
	expect(await highlightState(secondScreen)).toMatchObject({
		active: true,
		output_enabled: true,
	});

	// Closing one surface leaves the desk holding Highlight; the others are still standing at it.
	await secondScreen.request(
		"DELETE",
		`/api/v2/sessions/${secondScreen.session!.session_id}`,
	);
	expect((await highlightState(mainWindow)).active).toBe(true);

	// A surface that arrives later joins the Highlight that is already running rather than
	// contending for it — and rather than restarting it, so the activation basis it was turned on
	// with survives.
	const remembered = (await highlightState(mainWindow)).remembered;
	const lateSurface = new ApiDriver(api.baseUrl);
	await lateSurface.login("Operator");
	await replaceProgrammingSelection(lateSurface, {
		surface: "api",
		showId: show.id,
		fixtures: [fixtures[2].id],
	});
	await highlightAction(lateSurface, "on");
	expect(await highlightState(lateSurface)).toMatchObject({
		active: true,
		output_enabled: true,
		remembered,
	});
	expect(await highlightState(mainWindow)).toMatchObject({
		active: true,
		remembered,
	});
});

test("HIGHLIGHT-005 @supplemental-ui › Highlight errors remain reachable above production content without moving accepted controls", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await loadCanonicalCopy(api, bench, "highlight-005", "default-stage");
	const errors = [
		"The Highlight action was refused by the desk",
		"The Highlight action was rejected by the desk",
	];
	let nextHighlightError: string | null = null;
	await page.routeWebSocket("**/api/v2/events", (socket) => {
		const server = socket.connectToServer();
		socket.onMessage((message) => {
			const parsed = JSON.parse(String(message)) as {
				type?: string;
				action?: { type?: string };
				request_id?: string;
			};
			if (
				parsed.type === "action" &&
				parsed.action?.type === "highlight" &&
				parsed.request_id &&
				nextHighlightError
			) {
				socket.send(
					JSON.stringify({
						protocol_version: 2,
						request_id: parsed.request_id,
						ok: false,
						revision: 0,
						error: nextHighlightError,
					}),
				);
				nextHighlightError = null;
				return;
			}
			server.send(message);
		});
		server.onMessage((message) => socket.send(message));
	});

	for (const viewport of [
		{ width: 1280, height: 720 },
		{ width: 1600, height: 1100 },
	]) {
		await page.setViewportSize(viewport);
		await desk.open(bench.baseUrl);
		await openBuiltIn(page, "Fixtures");
		await expect(page.locator(".programmer-number-block")).toBeVisible();

		for (const errorMessage of errors) {
			const before = await softwareHighlightGeometry(page);
			nextHighlightError = errorMessage;
			await highlightKey(page, "HIGH").click();
			const alert = page.locator("[data-highlight-error-alert]");
			await expect(alert).toHaveCount(1);
			await expect(alert).toContainText(errorMessage);
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
			nextHighlightError = errors[0];
			await highlightKey(page, "HIGH").click();
			const alert = page.locator("[data-highlight-error-alert]");
			await expect(alert).toBeVisible();
			await hardware.subscribe(clientId, "desk");
			await expect
				.poll(
					async () =>
						(
							await api.request<any>(
								"GET",
								"/api/v2/bootstrap",
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
								"/api/v2/bootstrap",
								undefined,
								false,
							)
						).hardware_connected,
				)
				.toBe(false);
		}
	}
});
