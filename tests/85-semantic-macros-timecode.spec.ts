import type { Page, Route } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

test.describe("docs/testing/15-macros-and-timecode.md", () => {
	test("MACRO-003 @api › command-line start uses the shared one-shot execution", async ({
		api,
		show,
	}) => {
		const macroId = "00000000-0000-4000-8000-000000000071";
		await api.request(
			"POST",
			"/api/v2/macros/actions",
			{
				request_id: "playwright-create-macro-71",
				action: {
					type: "create",
					definition: {
						id: macroId,
						number: 71,
						name: "Clear programmer",
						source: "CLEAR",
						presentation: { color: "#8f3541" },
					},
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		const outcome = await api.request<{ outcome: string }>(
			"POST",
			"/api/v2/command-line/execute",
			{ request_id: "playwright-run-macro-71", command: "MACRO 71" },
			true,
			undefined,
			{ deskId: api.session?.desk.id },
		);
		expect(outcome.outcome).toBe("accepted");

		await expect
			.poll(async () => {
				const runtime = await api.request<{
					active: Array<{ macro_id: string; trigger: { type: string } }>;
					recent: Array<{
						macro_id: string;
						state: string;
						trigger: { type: string };
					}>;
				}>("GET", "/api/v2/macros/runtime", undefined, true, undefined, {
					showId: show.id,
					deskId: api.session?.desk.id,
				});
				return [...runtime.active, ...runtime.recent].find(
					(execution) => execution.macro_id === macroId,
				);
			})
			.toMatchObject({ macro_id: macroId, trigger: { type: "command_line" } });
	});

	test("MACRO-004 @ui › refined editor exposes settings, focus, alternating lines, and IntelliCode", async ({
		api,
		desk,
		page,
	}) => {
		const definition = {
			id: "00000000-0000-4000-8000-000000000160",
			number: 160,
			name: "Front wash",
			source: "F\nDEFINE _front FIXTURE 1\n_front",
			presentation: { color: "#315cab", icon: "play" },
		};
		await page.route("**/api/v2/objects/macro", async (route) => {
			await fulfillJson(route, {
				show_revision: 8,
				objects: [
					{
						kind: "macro",
						id: definition.id,
						revision: 4,
						updated_at: "2026-08-11T12:00:00Z",
						body: definition,
					},
				],
			});
		});
		await page.route("**/api/v2/macros/runtime", async (route) => {
			await fulfillJson(route, { desk_id: "desk-a", active: [], recent: [] });
		});
		await page.route("**/api/v2/macros/validate", async (route) => {
			await fulfillJson(route, {
				valid: true,
				diagnostics: [
					{
						line: 1,
						status: "valid",
						message: "Valid command",
						tokens: [],
					},
					{
						line: 2,
						status: "valid",
						message: "Valid command",
						tokens: [],
					},
					{
						line: 3,
						status: "valid",
						message: "Valid command",
						tokens: [
							{
								start: 0,
								end: 6,
								kind: "definition",
								expansion: "FIXTURE 1",
							},
						],
					},
				],
				suggestions: [
					{
						label: "FIXTURE",
						insert_text: "FIXTURE ",
						detail: "Select fixtures by number or range",
						replace_start: 0,
						replace_end: 1,
					},
				],
			});
		});

		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await openShiftedBuiltIn(page, "Macro");
		await page
			.getByRole("button", { name: "Macro 160 Front wash" })
			.click({ button: "right" });

		await expect(page.getByText("Macro", { exact: true }).first()).toBeVisible();
		await expect(page.getByText("Macro 160", { exact: true })).toHaveCount(0);
		await expect(page.getByRole("button", { name: "Run Macro" })).toContainText(
			"▶ Run Macro",
		);
		await expect(page.getByRole("button", { name: /copy/i })).toHaveCount(0);
		await page.getByRole("button", { name: "Settings" }).click();
		const settings = page.getByRole("dialog", { name: "Macro Settings" });
		await expect(settings.getByRole("textbox", { name: "Name" })).toHaveValue(
			"Front wash",
		);
		await expect(settings.getByText("Icon", { exact: true })).toBeVisible();
		await expect(settings.getByRole("button", { name: "Delete Macro" })).toBeVisible();
		await settings.getByRole("button", { name: "Close Macro Settings" }).click();

		const source = page.getByRole("textbox", { name: "Macro command lines" });
		await source.focus();
		await expect(page.locator(".macro-source-editor")).toHaveCSS(
			"outline-color",
			"rgb(55, 142, 255)",
		);
		await expect(page.locator(".macro-source-line.alternate")).toHaveCount(1);
		await expect(page.getByTitle("_front → FIXTURE 1")).toBeVisible();
		await page.getByRole("option", { name: /FIXTURE/ }).click();
		await expect(source).toHaveValue(
			"FIXTURE \nDEFINE _front FIXTURE 1\n_front",
		);
	});

	test("TIMECODE-003 @api › transport routes address one authoritative runtime", async ({
		api,
		show,
	}) => {
		const timecodeId = "00000000-0000-4000-8000-000000000070";
		await api.request(
			"POST",
			"/api/v2/timecodes/actions",
			{
				request_id: "playwright-create-timecode-70",
				action: {
					type: "create",
					definition: {
						id: timecodeId,
						number: 70,
						name: "Act one",
						duration_frame: 440,
						transport_offset_frame: 0,
						auto_start: false,
						audio: null,
						markers: [],
						lanes: [],
					},
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);
		const context = {
			showId: show.id,
			deskId: api.session?.desk.id,
		};
		const playing = await api.request<{ state: string; timecode_id: string }>(
			"POST",
			`/api/v2/timecodes/${timecodeId}/transport`,
			{ timecode_id: timecodeId, action: { type: "go" } },
			true,
			undefined,
			context,
		);
		expect(playing).toMatchObject({
			timecode_id: timecodeId,
			state: "playing",
		});
		const stopped = await api.request<{ state: string; frame: number }>(
			"POST",
			`/api/v2/timecodes/${timecodeId}/transport`,
			{ timecode_id: timecodeId, action: { type: "stop" } },
			true,
			undefined,
			context,
		);
		expect(stopped).toMatchObject({ state: "stopped", frame: 0 });
	});

	test("TIMECODE-001 @ui › title actions, Settings autosave, Add menu, CSV, and zoom geometry match the operator contract", async ({
		api,
		desk,
		page,
	}) => {
		const definition = editableTimecode();
		const mutations: Record<string, unknown>[] = [];
		let revision = 4;
		await page.route("**/api/v2/timecodes", async (route) => {
			await fulfillJson(route, {
				show_revision: 1,
				objects: [{ revision: 4, definition }],
			});
		});
		await page.route("**/api/v2/timecodes/runtime", async (route) => {
			await fulfillJson(route, []);
		});
		await page.route("**/api/v2/timecodes/actions", async (route) => {
			const mutation = route.request().postDataJSON() as Record<
				string,
				unknown
			>;
			mutations.push(mutation);
			revision += 1;
			await fulfillJson(route, {
				request_id: `saved-${revision}`,
				replayed: false,
				show_id: "00000000-0000-4000-8000-000000000001",
				show_revision: revision,
				object: {
					kind: "timecode",
					id: definition.id,
					revision,
					updated_at: "2026-08-11T00:00:00Z",
					body: definition,
				},
			});
		});

		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await openShiftedBuiltIn(page, "Timecode");
		await page
			.getByRole("button", { name: "Timecode 7 Opening track" })
			.click({ button: "right" });

		const editor = page.getByLabel("Timecode timeline editor");
		await expect(editor).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Save", exact: true }),
		).toHaveCount(0);
		for (const label of ["Play", "Pause", "Rewind to start", "Stop"])
			await expect(
				page.getByRole("button", { name: label, exact: true }),
			).toBeVisible();

		await page.getByRole("button", { name: "Add", exact: true }).click();
		const addMenu = page.getByRole("menu", { name: "Add" });
		await expect(addMenu.getByRole("menuitem")).toHaveText([
			"Add Marker",
			"Add Speed Lane",
			"Add Cuelist Lane",
		]);
		await expect(addMenu.getByText("Add Playhead")).toHaveCount(0);
		await addMenu.getByRole("menuitem", { name: "Add Speed Lane" }).click();
		// A Speed lane names its Speed Group before it exists, and Insert Keyframe appears only
		// once a lane is there to insert into.
		const speedGroups = page.getByRole("dialog", { name: "Choose Speed Group" });
		await speedGroups.getByRole("button", { name: "Add lane", exact: true }).click();
		await editor.getByRole("button", { name: /^Speed Group A · speed group/ }).click();
		await editor.getByRole("button", { name: "Insert Keyframe" }).click();

		const viewport = editor.getByLabel("Timecode timeline viewport");
		await expect
			.poll(() =>
				viewport.evaluate((node) => node.scrollWidth - node.clientWidth),
			)
			.toBeLessThanOrEqual(1);
		// Zoom lives in the overview window: dragging its end handle towards the start narrows the
		// visible zone, and the editor zooms to whatever that zone asks for, up to its maximum.
		const overview = editor.getByRole("scrollbar", {
			name: "Timeline overview",
		});
		const overviewBox = await overview.boundingBox();
		const endHandle = overview.getByRole("separator", {
			name: "Resize timeline overview from end",
		});
		const handleBox = await endHandle.boundingBox();
		if (!overviewBox || !handleBox) throw new Error("The overview is not visible");
		await page.mouse.move(
			handleBox.x + handleBox.width / 2,
			handleBox.y + handleBox.height / 2,
		);
		await page.mouse.down();
		await page.mouse.move(
			overviewBox.x + 1,
			handleBox.y + handleBox.height / 2,
			{ steps: 8 },
		);
		await page.mouse.up();
		await expect
			.poll(() =>
				editor
					.locator(".timecode-timeline-canvas")
					.getAttribute("data-pixels-per-frame"),
			)
			.toBe("17.5");

		await page.getByRole("button", { name: "Settings" }).click();
		const settings = page.getByRole("dialog", { name: "Timecode Settings" });
		await settings.getByLabel("Name").fill("Opening sequence");
		await settings.getByRole("button", { name: "Append" }).click();
		await page.getByRole("option", { name: "Replace" }).click();
		const configuration = await api.request<any>("GET", "/api/v2/configuration");
		await api.request("PUT", "/api/v2/configuration", {
			...configuration.configuration,
			file_manager_system_picker_fallback: true,
		});
		await settings
			.getByRole("button", { name: "Choose marker CSV" })
			.click();
		await page
			.getByRole("dialog", { name: "Choose files or folders" })
			.locator('input[type="file"]')
			.setInputFiles({
				name: "markers.csv",
				mimeType: "text/csv",
				buffer: Buffer.from(
					"position,name,color\n00:00:05:00,Verse,#a67cff",
				),
			});

		await expect
			.poll(() =>
				mutations.find((entry) =>
					JSON.stringify(entry).includes('"name":"Verse"'),
				),
			)
			.toBeTruthy();
		expect(mutations).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					action: expect.objectContaining({
						type: "update",
						timecode_id: definition.id,
						patch: {
							markers: [
								expect.objectContaining({
									frame: 220,
									name: "Verse",
									color: "#a67cff",
								}),
							],
						},
					}),
				}),
			]),
		);
		expect(mutations).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					action: expect.objectContaining({
						patch: expect.objectContaining({
							lanes: expect.arrayContaining([
								expect.objectContaining({
									content: expect.objectContaining({
										kind: "speed_group",
										keyframes: [
											expect.objectContaining({ bpm: 120, phase: 0 }),
										],
									}),
								}),
							]),
						}),
					}),
				}),
			]),
		);
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

function editableTimecode() {
	return {
		id: "00000000-0000-4000-8000-000000000077",
		number: 7,
		name: "Opening track",
		duration_frame: 44 * 30,
		transport_offset_frame: 0,
		auto_start: false,
		audio: null,
		markers: [],
		lanes: [
			{
				id: "00000000-0000-4000-8000-000000000078",
				name: "Main audio",
				content: {
					kind: "audio_volume",
					keyframes: [
						{
							id: "00000000-0000-4000-8000-000000000079",
							frame: 44,
							value: 1,
							fade_frames: 0,
							curve: "linear",
						},
					],
				},
			},
		],
	};
}

/** Macro and Timecode are Shift Built-ins, which the dock only shows while Shift is armed. */
async function openShiftedBuiltIn(page: Page, name: string): Promise<void> {
	const shift = page.locator('[data-keypad-key="SHIFT"]:visible').first();
	if (!(await shift.isVisible().catch(() => false)))
		await page.locator(".mode-toggle").click();
	await shift.click();
	await page
		.locator("[aria-label='Shift Built-ins']")
		.getByRole("button", { name, exact: true })
		.click();
}
