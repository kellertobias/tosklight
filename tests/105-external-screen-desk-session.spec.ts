import type { Page } from "@playwright/test";
import type { ScreenAttachment } from "../apps/light-desktop/src/api/client/screenAttachment";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { ControllableDesktopDriver } from "./bench/window-system/desktopBridge";

/* Where the desktop host writes the attachment; mirrors `SCREEN_ATTACHMENT_KEY` in windows.rs. */
const ATTACHMENT_KEY = "light.screen-attachment";
/* Nothing listens here, so a window that used it would fail to connect at all. */
const UNREACHABLE_SERVER = "http://127.0.0.1:9";

interface BootstrapClients {
	clients?: Array<{ client_id: string }>;
}

async function clientIds(api: ApiDriver): Promise<string[]> {
	const bootstrap = await api.request<BootstrapClients>(
		"GET",
		"/api/v2/bootstrap",
		undefined,
		false,
	);
	return (bootstrap.clients ?? []).map((client) => client.client_id).sort();
}

async function createOpenScreen(api: ApiDriver, screenId: string) {
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: {
			type: "create",
			configuration: {
				id: screenId,
				name: "Joined Screen",
				layout: { desks: [], activeDeskId: "main" },
				show_dock: false,
				show_playbacks: false,
				playback_count: 8,
				playback_rows: 1,
				first_playback_slot: 1,
				page_mode: "follow_main",
				show_page_controls: false,
				show_programmer: true,
				desired_open: true,
				display_id: null,
				bounds: null,
				fullscreen: false,
				playback_layout: null,
				content: { type: "control_surface" },
			},
		},
	});
	/* The screen carries the desk's encoders, so it shows the one desk command line too. */
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: {
			type: "update_programmer_control_surface",
			patch: { owner_screen_id: screenId, visible_encoders: 4 },
		},
	});
}

/**
 * A screen webview as the desktop host creates it: its own window storage holding only the
 * handed-over attachment (so no stored desk session to fall back on), a native desktop webview,
 * and an operator setting that points elsewhere so a window that looked for its own server would
 * visibly go wrong.
 */
async function openScreenWindow(
	page: Page,
	origin: string,
	screenId: string,
	attachment: ScreenAttachment,
) {
	const browser = page.context().browser();
	if (!browser) throw new Error("A screen window needs its own browser context");
	const context = await browser.newContext({
		viewport: page.viewportSize() ?? undefined,
	});
	const screen = await context.newPage();
	await new ControllableDesktopDriver(screen).install({ nativeWebview: true });
	await screen.addInitScript(
		({ key, value, unreachable }) => {
			if (sessionStorage.getItem(key) === null)
				sessionStorage.setItem(key, value);
			localStorage.setItem("light.server-url", unreachable);
		},
		{
			key: ATTACHMENT_KEY,
			value: JSON.stringify(attachment),
			unreachable: UNREACHABLE_SERVER,
		},
	);
	const requests: Array<{ method: string; url: string }> = [];
	screen.on("request", (request) =>
		requests.push({ method: request.method(), url: request.url() }),
	);
	await screen.goto(`${origin}/?screen=${encodeURIComponent(screenId)}`);
	return { screen, requests };
}

test("TL-470 @ui › Open Screen joins the desk's own server and session instead of starting a second desk", async ({
	api,
	bench,
	desk,
	page,
}) => {
	const control = await desk.enableControllableDesktop();
	await desk.open(bench.baseUrl);
	const session = await desk.session();
	api.session = session;
	const origin = new URL(bench.baseUrl).origin;
	const screenId = crypto.randomUUID();
	await createOpenScreen(api, screenId);

	await expect
		.poll(
			() =>
				control.actions.some(
					(action) =>
						action.type === "open_console_screen" &&
						action.screen.screenId === screenId &&
						action.screen.attachment !== null,
				),
			{ timeout: 10_000 },
		)
		.toBe(true);
	const open = control.actions.find(
		(action) =>
			action.type === "open_console_screen" &&
			action.screen.screenId === screenId &&
			action.screen.attachment !== null,
	);
	if (open?.type !== "open_console_screen" || !open.screen.attachment)
		throw new Error("Open Screen handed over no desk attachment");
	const attachment = open.screen.attachment;
	expect(attachment.server_url).toBe(origin);
	expect(attachment.session.session_id).toBe(session.session_id);
	expect(attachment.session.token).toBe(session.token);

	const clientsBefore = await clientIds(api);
	const { screen, requests } = await openScreenWindow(
		page,
		origin,
		screenId,
		attachment,
	);
	try {
		const screenCommandLine = screen.getByRole("textbox", {
			name: "Command line",
			exact: true,
		});
		await expect(screenCommandLine).toHaveValue("FIXTURE", {
			timeout: 10_000,
		});

		/* The screen follows the one desk command line live, typed on the initiating window. */
		for (const key of ["1", "AT"])
			await page.locator(`[data-keypad-key="${key}"]`).click();
		await expect(screenCommandLine).toHaveValue(/1 AT$/);
		await expect(
			page.getByRole("textbox", { name: "Command line", exact: true }),
		).toHaveValue(/1 AT$/);

		/* One server, one desk session: the screen never logged in or reached another server. */
		expect(
			requests.filter((request) => request.url.startsWith(UNREACHABLE_SERVER)),
		).toEqual([]);
		expect(
			requests.filter(
				(request) =>
					request.method === "POST" &&
					new URL(request.url).pathname === "/api/v2/sessions",
			),
		).toEqual([]);
		expect(await clientIds(api)).toEqual(clientsBefore);
		expect(await desk.session()).toMatchObject({
			session_id: session.session_id,
		});
	} finally {
		await screen.context().close();
	}
});

test("TL-470 @ui › a screen that cannot reach the desk's server says why and retries without starting a desk", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.open(bench.baseUrl);
	const session = await desk.session();
	api.session = session;
	const origin = new URL(bench.baseUrl).origin;
	const clientsBefore = await clientIds(api);
	const { screen, requests } = await openScreenWindow(
		page,
		origin,
		crypto.randomUUID(),
		{ server_url: UNREACHABLE_SERVER, session, desk_token: null },
	);
	try {
		await expect(
			screen.getByRole("heading", { name: "Screen cannot join the desk" }),
		).toBeVisible({ timeout: 10_000 });
		await expect(screen.getByRole("alert")).toContainText(
			`${UNREACHABLE_SERVER} is not reachable`,
		);
		await expect(screen.getByLabel("Light server URL")).toHaveCount(0);

		const retry = screen.getByRole("button", { name: "Retry now" });
		const attemptsBefore = requests.length;
		await retry.click();
		await retry.click();
		await expect
			.poll(() => requests.length, { timeout: 5_000 })
			.toBeGreaterThan(attemptsBefore);
		await expect(retry).toBeVisible();

		expect(
			requests.filter(
				(request) =>
					new URL(request.url).pathname === "/api/v2/sessions" ||
					(request.url.startsWith(origin) &&
						new URL(request.url).pathname.startsWith("/api/")),
			),
		).toEqual([]);
		expect(await clientIds(api)).toEqual(clientsBefore);
	} finally {
		await screen.context().close();
	}
});
