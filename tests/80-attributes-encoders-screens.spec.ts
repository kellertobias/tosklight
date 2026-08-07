import type { ApiDriver, Session } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { fixtureIdsByNumber, loadCanonicalCopy } from "./support/catalog";

interface ProgrammerValueEntry {
	fixture_id: string;
	attribute: string;
	value: unknown;
}

interface ProgrammerProjection {
	session_id: string;
	command_line: string;
	values: ProgrammerValueEntry[];
}

test.describe("TL-65 attributes, encoders, and screens", () => {
	test("TL-65 @ui › assigned browser control screen reuses the desk command line and programmer", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await loadCanonicalCopy(api, bench, "tl-65-browser-control");
		await desk.open(bench.baseUrl);
		const session = await desk.session();
		api.session = session;
		const screenId = crypto.randomUUID();
		const hardware = await bench.osc();
		const browserScreen = await page.context().newPage();

		try {
			await expect(page.getByLabel("Command line")).toBeVisible();
			await configureBrowserControlScreen(api, screenId);
			await browserScreen.goto(`${bench.baseUrl}?screen=${screenId}`);
			const commandLine = browserScreen.getByLabel("Command line");
			await expect(commandLine).toHaveValue("FIXTURE", { timeout: 10_000 });
			/* The main screen keeps its own command line and keypad; only the encoders move. */
			await expect(page.getByLabel("Command line")).toBeVisible();
			expect(await sessionProgrammerCount(api, session)).toBe(1);

			await updateProgrammerControlSurface(api, {
				owner_screen_id: screenId,
				visible_encoders: 6,
			});
			/* Nothing is selected yet, so the encoder surface carries its empty state. */
			await expect(browserScreen.locator(".parameter-surfaces")).toHaveCount(1);
			await expect(browserScreen.locator(".parameter-empty")).toBeVisible();
			await updateProgrammerControlSurface(api, { assign_to_main: true });
			await expect(page.getByLabel("Command line")).toBeVisible();
			await expect(browserScreen.locator(".parameter-surfaces")).toHaveCount(0);
			await updateProgrammerControlSurface(api, {
				owner_screen_id: screenId,
				visible_encoders: 4,
			});
			await expect(commandLine).toHaveValue("FIXTURE");

			/* The keypad stays on the main screen wherever the encoders sit. */
			await expect(
				browserScreen.locator("[data-keypad-key]"),
			).toHaveCount(0);
			for (const key of ["1", "AT", "5", "0", "ENT"])
				await page.locator(`[data-keypad-key="${key}"]`).click();
			const fixtures = await fixtureIdsByNumber(api);
			await expect
				.poll(() => fixtureProgrammerValue(api, fixtures[1], "intensity"))
				.toBeCloseTo(0.5, 4);

			const encoder = browserScreen.getByRole("group", {
				name: "Enc 1 · Dimmer",
				exact: true,
			});
			await expect(encoder).toBeVisible();
			await expect(
				browserScreen.locator(".parameter-surfaces > *"),
			).toHaveCount(4);
			const encoderValue = encoder.getByRole("button", {
				name: "Set Enc 1 · Dimmer value",
			});
			await expect(encoderValue).toHaveText("50%");

			await encoder.locator(".touch-encoder-tap-positive").click();
			await expect
				.poll(() => fixtureProgrammerValue(api, fixtures[1], "intensity"))
				.toBeCloseTo(0.501, 4);

			await encoder.dispatchEvent("wheel", { deltaY: -1, shiftKey: true });
			await expect
				.poll(() => fixtureProgrammerValue(api, fixtures[1], "intensity"))
				.toBeCloseTo(0.511, 4);

			await encoderValue.click();
			const editor = browserScreen.getByRole("dialog", {
				name: "Enc 1 · Dimmer value",
			});
			await expect(editor).toBeVisible();
			for (const key of ["7", "5", "ENTER"])
				await editor.getByRole("button", { name: key, exact: true }).click();
			await expect
				.poll(() => fixtureProgrammerValue(api, fixtures[1], "intensity"))
				.toBeCloseTo(0.75, 4);

			await api.setCommandLineText("GROUP 1 +");
			await expect(commandLine).toHaveValue("GROUP 1 +");
			await hardware.subscribe(
				`tl-65-browser-${crypto.randomUUID()}`,
				session.desk.osc_alias,
			);
			await hardware.send(
				`/light/${session.desk.osc_alias}/programmer/digit-2`,
				[true],
			);
			await expect(commandLine).toHaveValue("GROUP 1 + F2");
			await expect
				.poll(() => programmerCommand(api, session))
				.toBe("GROUP 1 + F2");
			expect(await sessionProgrammerCount(api, session)).toBe(1);

			await browserScreen.reload();
			await expect(browserScreen.getByLabel("Command line")).toHaveValue(
				"GROUP 1 + F2",
			);
			expect(await sessionProgrammerCount(api, session)).toBe(1);

			await deleteScreen(api, screenId);
			await expect(browserScreen.getByRole("alert")).toContainText(
				"Screen unavailable",
			);
			await expect(browserScreen.getByLabel("Command line")).toHaveCount(0);
		} finally {
			await hardware.close();
			await browserScreen.close();
		}
	});
});

async function configureBrowserControlScreen(
	api: ApiDriver,
	screenId: string,
): Promise<void> {
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: {
			type: "create",
			configuration: {
				id: screenId,
				name: "Browser Programmer",
				layout: { desks: [], activeDeskId: "main" },
				show_dock: false,
				show_playbacks: false,
				playback_count: 8,
				playback_rows: 1,
				first_playback_slot: 1,
				page_mode: "follow_main",
				show_page_controls: false,
				show_programmer: true,
				desired_open: false,
				display_id: null,
				bounds: null,
				fullscreen: false,
				playback_layout: null,
				content: { type: "control_surface" },
			},
		},
	});
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: {
			type: "update_programmer_control_surface",
			patch: { owner_screen_id: screenId, visible_encoders: 4 },
		},
	});
}

async function updateProgrammerControlSurface(
	api: ApiDriver,
	patch:
		| { owner_screen_id: string; visible_encoders?: 4 | 6 }
		| { assign_to_main: true },
): Promise<void> {
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: {
			type: "update_programmer_control_surface",
			patch,
		},
	});
}

async function deleteScreen(api: ApiDriver, screenId: string): Promise<void> {
	await api.request("POST", "/api/v2/screens/actions", {
		request_id: crypto.randomUUID(),
		action: { type: "delete", screen_id: screenId },
	});
}

async function fixtureProgrammerValue(
	api: ApiDriver,
	fixtureId: string,
	attribute: string,
): Promise<number | null> {
	const states = await api.request<ProgrammerProjection[]>(
		"GET",
		"/api/v2/programmers",
	);
	const value = states
		.flatMap((state) => state.values)
		.find(
			(entry) =>
				entry.fixture_id === fixtureId && entry.attribute === attribute,
		);
	let current: unknown = value;
	while (current && typeof current === "object" && "value" in current)
		current = (current as { value: unknown }).value;
	return typeof current === "number" ? current : null;
}

async function programmerCommand(
	api: ApiDriver,
	session: Session,
): Promise<string | null> {
	const states = await api.request<ProgrammerProjection[]>(
		"GET",
		"/api/v2/programmers",
	);
	return (
		states
			.find((state) => state.session_id === session.session_id)
			?.command_line?.trim() ?? null
	);
}

async function sessionProgrammerCount(
	api: ApiDriver,
	session: Session,
): Promise<number> {
	const states = await api.request<ProgrammerProjection[]>(
		"GET",
		"/api/v2/programmers",
	);
	return states.filter((state) => state.session_id === session.session_id)
		.length;
}
