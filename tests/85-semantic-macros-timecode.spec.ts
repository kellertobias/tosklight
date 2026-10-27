import type { Locator, Page, Route } from "@playwright/test";
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

	test("MACRO-002 @api › a running Macro never disturbs the command line the operator is typing", async ({
		api,
		show,
	}) => {
		const macroId = "00000000-0000-4000-8000-000000000072";
		await api.request(
			"POST",
			"/api/v2/macros/actions",
			{
				request_id: "playwright-create-macro-72",
				action: {
					type: "create",
					definition: {
						id: macroId,
						number: 72,
						name: "Detached lines",
						source: "1 AT 50",
						presentation: { color: "#3f6f45" },
					},
				},
			},
			true,
			undefined,
			{ showId: show.id },
		);

		const typed = await api.setCommandLineText("GROUP 1 AT 50");
		expect(typed.commandLine).toMatchObject({
			text: "GROUP 1 AT 50",
			pristine: false,
		});

		await api.request(
			"POST",
			`/api/v2/macros/${macroId}/run`,
			{ request_id: "playwright-run-macro-72" },
			true,
			undefined,
			{ showId: show.id, deskId: api.session?.desk.id },
		);
		await expect
			.poll(async () => {
				const runtime = await api.request<{
					recent: Array<{ macro_id: string; state: string }>;
				}>("GET", "/api/v2/macros/runtime", undefined, true, undefined, {
					showId: show.id,
					deskId: api.session?.desk.id,
				});
				return runtime.recent.find(
					(execution) => execution.macro_id === macroId,
				)?.state;
			})
			.toBe("succeeded");

		// The Macro line reached the Programmer …
		const programmers = await api.request<Array<{ values: unknown[] }>>(
			"GET",
			"/api/v2/programmers",
		);
		expect(programmers.some((state) => state.values.length > 0)).toBe(true);

		// … but the half-entered operator command is exactly as it was typed — same text, same
		// revision, so no command-line change was published either.
		expect((await api.getCommandLine()).commandLine).toMatchObject({
			text: "GROUP 1 AT 50",
			pristine: false,
			revision: typed.commandLine.revision,
		});
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
		await page.getByRole("button", { name: "Settings", exact: true }).click();
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

	test("TIMECODE-001 @ui › live editing preserves transport and scrubbing stays independent until explicit seek", async ({ api, show, desk, page, bench }, testInfo) => {
		const definition = editableTimecode();
		await api.request("POST", "/api/v2/timecodes/actions", {
			request_id: "create-live-editor-timecode",
			action: { type: "create", definition },
		}, true, undefined, { showId: show.id });
		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await openShiftedBuiltIn(page, "Timecode");
		await page.getByRole("button", { name: "Timecode 7 Opening track" }).click({ button: "right" });
		await expect(page.getByText("Saved", { exact: true })).toBeVisible();
		await page.getByRole("button", { name: "Play", exact: true }).click();
		const runtime = () => api.request<{ state: string; frame: number }>("GET", `/api/v2/timecodes/${definition.id}/runtime`);
		await expect.poll(async () => (await runtime()).state).toBe("playing");
		await bench.tick(1000);
		await expect.poll(async () => (await runtime()).frame).toBeGreaterThan(10);
		await page.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Timecode Settings" });
		await settings.getByLabel("Show selection sidebar", { exact: true }).check();
		await settings.getByLabel("Name", { exact: true }).fill("Live edit keeps playing");
		await expect(page.getByText("Saved", { exact: true })).toBeVisible();
		expect((await runtime()).state).toBe("playing");
		// Typing is intentionally character-by-character, including incomplete intermediate text.
		const duration = settings.getByLabel("Duration", { exact: true });
		await duration.fill("");
		await duration.pressSequentially("00:00:45.00");
		await duration.press("Tab");
		await expect(duration).toHaveValue("00:00:45.00");
		await settings.getByRole("button", { name: /Close/ }).click();
		await expect(page.getByRole("button", { name: "Seek to playhead", exact: true })).toHaveCount(0);
		await expect(page.getByLabel("Follow transport", { exact: true })).toHaveCount(0);
		const canvas = page.locator(".timecode-timeline-canvas");
		const bounds = await canvas.boundingBox();
		if (!bounds) throw new Error("Timeline canvas is not visible");
		await page.mouse.click(bounds.x + 350, bounds.y + 12);
		const playheadValue = page.getByRole("button", { name: "Drag playhead to seek", exact: true });
		const position = page.getByRole("button", { name: "Timecode position", exact: true });
		const target = await playheadValue.innerText();
		const before = (await runtime()).frame;
		await bench.tick(1000);
		await expect.poll(async () => (await runtime()).frame).toBeGreaterThan(before + 5);
		await expect(playheadValue).toHaveText(target);
		await page.getByRole("button", { name: "Pause", exact: true }).click();
		await expect.poll(async () => (await runtime()).state).toBe("paused");
		await page.mouse.click(bounds.x + 350, bounds.y + 12);
		const seekTarget = await playheadValue.innerText();
		await position.click();
		const [hours, minutes, seconds, frames] = seekTarget.trim().split(/[:.]/).map(Number);
		const targetFrame = ((hours * 60 + minutes) * 60 + seconds) * 44 + frames;
		await expect.poll(async () => (await runtime()).frame).toBe(targetFrame);
		await page.getByRole("button", { name: "Add", exact: true }).click();
		await page.getByRole("menuitem", { name: "Add Speed Lane", exact: true }).click();
		await page.getByRole("dialog", { name: "Choose Speed Group" }).getByRole("button", { name: "Add lane", exact: true }).click();
		await expect(page.getByLabel("Phase", { exact: true })).toBeVisible();
		expect((await page.getByLabel("Phase", { exact: true }).boundingBox())?.width).toBeGreaterThan(48);
		await expect(page.getByRole("button", { name: "Next Keyframe", exact: true })).toBeInViewport();
		await expect(page.getByRole("button", { name: "Copy keyframe", exact: true })).toBeInViewport();
		await page.screenshot({ path: testInfo.outputPath("timecode-editor.png") });
		await page.getByRole("button", { name: "Stop", exact: true }).click();
		await expect.poll(async () => (await runtime()).state).toBe("stopped");
	});

	test("TIMECODE-001 @ui › title actions, Settings autosave, Add menu, CSV, and zoom geometry match the operator contract", async ({
		api,
		desk,
		page,
	}, testInfo) => {
		const definition = { ...editableTimecode(), duration_frame: 44 * 61 + 13 };
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
		await assertRulerLabelsDoNotOverlap(editor, definition.duration_frame);
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-ruler-fit-detail.png") });
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
		await editor.getByRole("button", { name: /^Speed Group A ·/ }).click();
		await page.getByRole("button", { name: "Insert Keyframe", exact: true }).click();

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
		await assertRulerLabelsDoNotOverlap(editor, definition.duration_frame);
		const scrollBounds = (await viewport.boundingBox())!;
		await page.mouse.move(scrollBounds.x + scrollBounds.width / 2, scrollBounds.y + 20);
		await page.mouse.wheel(100000, 0);
		await expect(editor.locator(".timecode-ruler-final-tick")).toBeInViewport();
		await assertRulerLabelsDoNotOverlap(editor, definition.duration_frame);
		await page.locator(".timecode-window.timecode-editor").screenshot({ path: testInfo.outputPath("timecode-ruler-right-edge-detail.png") });
		await fitTimelineOverview(page);


		await page.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Timecode Settings" });
		// The settings are tabbed: the name is on Generic, the marker import on Markers.
		await settings.getByLabel("Name").fill("Opening sequence");
		await settings.getByRole("tab", { name: "Markers" }).click();
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

test("TIMECODE-001 @ui › audio volume points and waveform share exact axes at Fit and through sidebar changes", async ({ api, desk, page }) => {
	const source = editableTimecode();
	const frames = [44, 660, 1276];
	const definition = {
		...source,
		audio: { asset_id: "00000000-0000-4000-8000-000000000090", asset_revision: 1, file_name: "Alignment.wav" },
		lanes: [{ ...source.lanes[0], content: { kind: "audio_volume", keyframes: frames.map((frame, index) => ({
			id: `00000000-0000-4000-8000-00000000009${index + 1}`, frame, value: index / 2, fade_frames: 0, curve: "linear",
		})) } }, {
			id: "00000000-0000-4000-8000-000000000095", name: "Movement", content: {
				kind: "speed_group", group: "A", keyframes: frames.map((frame, index) => ({
					id: `00000000-0000-4000-8000-00000000009${index + 6}`, frame, bpm: 60 + index * 60, phase: 0,
				})),
			},
		}],
	};
	await page.route("**/api/v2/timecodes", (route) => fulfillJson(route, { show_revision: 1, objects: [{ revision: 4, definition }] }));
	await page.route("**/api/v2/timecodes/runtime", (route) => fulfillJson(route, []));
	await page.route("**/api/v2/timecodes/*/audio/waveform", (route) => fulfillJson(route, { peaks: [0, 0.5, 1, 0.5, 0] }));
	await desk.open(api.baseUrl);
	await openBuiltIns(page);
	await openShiftedBuiltIn(page, "Timecode");
	await page.getByRole("button", { name: "Timecode 7 Opening track" }).click({ button: "right" });
	const editor = page.getByLabel("Timecode timeline editor", { exact: true });
	const audio = editor.locator(".main-audio.lane-audio_volume");
	await expect(audio.locator(".timecode-waveform path")).toHaveAttribute("d", /.+/);
	const speedGroups = editor.getByRole("button", { name: "Speed Groups", exact: true });
	if (await speedGroups.getAttribute("aria-expanded") === "false") await speedGroups.click();
	await fitTimelineOverview(page);
	await assertAudioAxes(editor, frames, definition.duration_frame);
	await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
 await page.getByRole("button", { name: "Settings", exact: true }).click();
 const settings = page.getByRole("dialog", { name: "Timecode Settings" });
 await settings.getByLabel("Show selection sidebar", { exact: true }).check();
 await settings.getByRole("button", { name: /Close/ }).click();
 await audio.locator(".item-volume").nth(1).click();
	await expect(editor.locator(".timecode-editor-inspector")).toBeVisible();
	await assertAudioAxes(editor, frames, definition.duration_frame);
	const overview = editor.getByRole("scrollbar", { name: "Timeline overview" });
	const grip = (await overview.getByRole("separator", { name: "Resize timeline overview from end" }).boundingBox())!;
	const track = (await overview.boundingBox())!;
	await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
	await page.mouse.down();
	await page.mouse.move(track.x + track.width / 2, grip.y + grip.height / 2, { steps: 8 });
	await page.mouse.up();
	await assertAudioAxes(editor, frames, definition.duration_frame);
	const viewport = editor.getByLabel("Timecode timeline viewport", { exact: true });
	const bounds = (await viewport.boundingBox())!;
	await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + 50);
	await page.mouse.wheel(100_000, 0);
	// Native end-of-scroll is authoritative: scrollWidth can include reserved
	// scrollbar gutter or endpoint paint beyond the actual timeline canvas.
	await expect.poll(async () => {
		const left = await viewport.evaluate((node) => node.scrollLeft);
		await page.waitForTimeout(100);
		return viewport.evaluate((node, previous) => {
			const canvas = node.querySelector<HTMLElement>(".timecode-timeline-canvas")!;
			const right = node.getBoundingClientRect().left + node.clientLeft + node.clientWidth;
			return { settled: node.scrollLeft === previous, endingVisible: canvas.getBoundingClientRect().right <= right + 1 };
		}, left);
	}).toEqual({ settled: true, endingVisible: true });
	const reachedEnd = await viewport.evaluate((node) => node.scrollLeft);
	// A second physical wheel must not move even one pixel farther. This proves
	// the native end, without assuming the gutter is scrollable time-axis space.
	await page.mouse.wheel(100_000, 0);
	await page.waitForTimeout(100);
	await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBe(reachedEnd);
	console.log("Timecode native right edge", await viewport.evaluate((node) => ({
		scrollLeft: node.scrollLeft, scrollWidth: node.scrollWidth, clientWidth: node.clientWidth,
		offsetWidth: (node as HTMLElement).offsetWidth, clientLeft: node.clientLeft,
		scrollbarGutter: getComputedStyle(node).scrollbarGutter,
		canvasWidth: node.querySelector<HTMLElement>(".timecode-timeline-canvas")!.getBoundingClientRect().width,
	})));
 await expect.poll(() => viewport.evaluate((node) => node.scrollLeft)).toBeGreaterThan(0);
	await assertAudioAxes(editor, frames, definition.duration_frame);
	const before = await viewport.evaluate((node) => node.scrollLeft);
	await editor.getByRole("button", { name: "Close selection settings", exact: true }).click();
	await expect(editor.locator(".timecode-editor-inspector")).toHaveCount(0);
	await assertAudioAxes(editor, frames, definition.duration_frame);
	expect(await viewport.evaluate((node) => node.scrollLeft)).toBe(before);
});

async function assertAudioAxes(editor: Locator, frames: number[], duration: number) {
	await expect.poll(() => editor.locator(".main-audio.lane-audio_volume").evaluate((lane, expected) => {
		const svg = lane.querySelector<SVGSVGElement>(".timecode-audio-keyframe-curve")!;
		const line = svg.querySelector("polyline")!;
		const matrix = svg.getScreenCTM()!;
		const buttons = [...lane.querySelectorAll<HTMLElement>(".item-volume")];
		const canvas = lane.closest<HTMLElement>(".timecode-timeline-canvas")!;
		const scale = Number(canvas.dataset.pixelsPerFrame);
		const origin = canvas.getBoundingClientRect().left + Number(canvas.dataset.timeOriginPx);
		let error = Math.max(
			Math.abs(svg.getBoundingClientRect().width - expected.duration * scale),
			Math.abs(lane.querySelector(".timecode-waveform")!.getBoundingClientRect().width - expected.duration * scale),
		);
		for (const [index, button] of buttons.entries()) {
			const point = svg.createSVGPoint();
			point.x = line.points.getItem(index).x;
			point.y = line.points.getItem(index).y;
			const projected = point.matrixTransform(matrix);
			const box = button.getBoundingClientRect();
			const x = box.left + box.width / 2;
			const y = box.top + box.height / 2;
			error = Math.max(error, Math.abs(projected.x - x), Math.abs(projected.y - y), Math.abs(x - origin - expected.frames[index] * scale));
		}
		const speed = canvas.querySelector(".lane-speed_group")!;
		const speedSvg = speed.querySelector<SVGSVGElement>(".timecode-speed-keyframe-curve")!;
		const speedLine = speedSvg.querySelector("polyline")!;
		const speedButtons = [...speed.querySelectorAll<HTMLElement>(".item-speed")];
		error = Math.max(error, Math.abs(speedSvg.getBoundingClientRect().width - expected.duration * scale));
		for (const [index, button] of speedButtons.entries()) {
			const point = speedSvg.createSVGPoint();
			const vertex = speedLine.points.getItem(index * 2 + 1);
			point.x = vertex.x; point.y = vertex.y;
			const projected = point.matrixTransform(speedSvg.getScreenCTM()!);
			const box = button.getBoundingClientRect();
			error = Math.max(error, Math.abs(projected.x - box.left - box.width / 2), Math.abs(projected.y - box.top - box.height / 2));
		}
		return buttons.length === expected.frames.length && speedButtons.length === expected.frames.length ? error : Number.MAX_SAFE_INTEGER;
	}, { frames, duration })).toBeLessThanOrEqual(0.75);
}

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

async function assertRulerLabelsDoNotOverlap(editor: import("@playwright/test").Locator, duration: number) {
	const ruler = editor.locator(".timecode-ruler");
	await expect(ruler.locator(".timecode-ruler-final-tick")).toHaveAttribute("data-frame", String(duration));
	const labels = await ruler.locator(":scope > span").evaluateAll((nodes) => nodes.map((node) => {
		const box = node.getBoundingClientRect();
		return { left: box.left, right: box.right };
	}));
	for (let index = 1; index < labels.length; index += 1) expect(labels[index].left - labels[index - 1].right).toBeGreaterThanOrEqual(1);
	const endpoint = await ruler.locator(".timecode-ruler-final-tick").boundingBox();
	const origin = (await ruler.boundingBox())!.x;
	const scale = Number(await editor.locator(".timecode-timeline-canvas").getAttribute("data-pixels-per-frame"));
	expect(Math.abs(endpoint!.x + endpoint!.width - origin - duration * scale)).toBeLessThanOrEqual(1);
}

/** Fit via the operator overview handles; the title deliberately has no Fit button. */
async function fitTimelineOverview(page: Page) {
 const overview = page.getByRole("scrollbar", { name: "Timeline overview" });
 const box = (await overview.boundingBox())!;
 for (const [side, x] of [["start", box.x + 1], ["end", box.x + box.width - 1]] as const) {
  const handle = (await overview.getByRole("separator", { name: `Resize timeline overview from ${side}` }).boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(x, handle.y + handle.height / 2, { steps: 8 });
  await page.mouse.up();
 }
}
