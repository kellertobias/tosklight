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

	test("TIMECODE-001 @ui › production editor zooms, imports markers, edits keyframes, and saves one revision-safe object", async ({
		api,
		desk,
		page,
	}) => {
		const definition = editableTimecode();
		let mutation: Record<string, unknown> | null = null;
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
			mutation = route.request().postDataJSON() as Record<string, unknown>;
			await fulfillJson(route, { request_id: "saved", replayed: false });
		});

		await desk.open(api.baseUrl);
		await expect(page.locator(".connection-cover")).toBeHidden();
		await openBuiltIns(page);
		await page
			.locator("[aria-label='Built-ins']")
			.getByRole("button", { name: "Timecode", exact: true })
			.click();
		await page
			.getByRole("button", { name: "Timecode 7 Opening track" })
			.click();

		const editor = page.getByLabel("Timecode timeline editor");
		await expect(editor).toBeVisible();
		await editor.getByLabel("Timeline zoom").fill("2");
		await editor.getByRole("button", { name: "Add speed lane" }).click();
		await editor.getByRole("button", { name: "+ keyframe" }).last().click();
		await editor
			.getByRole("button", { name: "Add marker at playhead" })
			.click();
		await editor.getByRole("button", { name: "Import marker CSV" }).click();
		await editor
			.getByLabel("Marker CSV")
			.fill("position,name,color\n00:00:05:00,Verse,#a67cff");
		await editor.getByLabel("Import mode").selectOption("replace");
		await editor.getByRole("button", { name: "Apply marker CSV" }).click();
		await page.getByRole("button", { name: "Save", exact: true }).click();

		await expect.poll(() => mutation).not.toBeNull();
		expect(mutation).toMatchObject({
			action: {
				type: "update",
				timecode_id: definition.id,
				expected_revision: 4,
				patch: {
					markers: [
						expect.objectContaining({
							frame: 220,
							name: "Verse",
							color: "#a67cff",
						}),
					],
					lanes: expect.arrayContaining([
						expect.objectContaining({
							content: expect.objectContaining({
								kind: "speed_group",
								keyframes: [expect.objectContaining({ bpm: 120, phase: 0 })],
							}),
						}),
					]),
				},
			},
		});
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
