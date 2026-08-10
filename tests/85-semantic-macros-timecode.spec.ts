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
});
