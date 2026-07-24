import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultServerUrl, LightApiClient } from "./LightApiClient";

beforeEach(() => {
	const values = new Map<string, string>();
	const sessionValues = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
		removeItem: (key: string) => values.delete(key),
		clear: () => values.clear(),
	});
	vi.stubGlobal("sessionStorage", {
		getItem: (key: string) => sessionValues.get(key) ?? null,
		setItem: (key: string, value: string) => sessionValues.set(key, value),
		removeItem: (key: string) => sessionValues.delete(key),
		clear: () => sessionValues.clear(),
	});
});
afterEach(() => vi.unstubAllGlobals());

describe("LightApiClient server selection and sessions", () => {
	it("uses same-origin in a browser and the loopback daemon in Tauri", () => {
		expect(
			defaultServerUrl(new URL("http://desk.local/") as unknown as Location),
		).toBe("http://desk.local");
		expect(
			defaultServerUrl(new URL("tauri://localhost/") as unknown as Location),
		).toBe("http://127.0.0.1:5000");
	});

	it("uses a session-only desktop test server without replacing the saved operator server", () => {
		localStorage.setItem("light.server-url", "http://desk.local:5000");
		sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:64649");
		expect(
			defaultServerUrl(new URL("tauri://localhost/") as unknown as Location),
		).toBe("http://127.0.0.1:64649");
		expect(localStorage.getItem("light.server-url")).toBe(
			"http://desk.local:5000",
		);
	});

	it("keeps desktop test identity and desk state out of persistent storage", async () => {
		sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:64649");
		vi.stubGlobal(
			"fetch",
			vi.fn().mockResolvedValue(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
						desk: { id: "desk-a" },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			),
		);
		await new LightApiClient().login("Operator");
		expect(sessionStorage.getItem("light.client-id")).toEqual(
			expect.any(String),
		);
		expect(sessionStorage.getItem("light.control-desk")).toBe("desk-a");
		expect(localStorage.getItem("light.client-id")).toBeNull();
		expect(localStorage.getItem("light.control-desk")).toBeNull();
	});

	it("creates a username session and authenticates subsequent REST requests", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "secret-token",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ screens: [], active_pages: {} }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");

		await client.login("Operator");
		await client.screens();

		expect(fetchMock.mock.calls[0][0]).toBe(
			"http://desk.local/api/v2/sessions",
		);
		expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual(
			expect.objectContaining({
				username: "Operator",
				client_id: expect.any(String),
			}),
		);
		const authenticatedHeaders = fetchMock.mock.calls[1][1].headers as Headers;
		expect(authenticatedHeaders.get("authorization")).toBe(
			"Bearer secret-token",
		);
	});

	it("uses v2 bootstrap, Patch, and session-close runtime contracts", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ api_version: "v2" }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						client_id: "client-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
						desk: { id: "desk-a" },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						show_id: "00000000-0000-4000-8000-000000000001",
						show_revision: 4,
						patch_revision: 3,
						cursor: { sequence: 9 },
						fixtures: [],
						profile_revisions: [],
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");

		await expect(client.bootstrap()).resolves.toMatchObject({
			api_version: "v2",
		});
		await client.login("Operator");
		await expect(client.patch()).resolves.toEqual({
			showId: "00000000-0000-4000-8000-000000000001",
			showRevision: 4,
			patchRevision: 3,
			cursor: 9,
			fixtures: [],
			profileRevisions: [],
		});
		await client.closeSession();

		expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
			"http://desk.local/api/v2/bootstrap",
			"http://desk.local/api/v2/sessions",
			"http://desk.local/api/v2/patch",
			"http://desk.local/api/v2/sessions/session-a",
		]);
		const patchHeaders = fetchMock.mock.calls[2][1].headers as Headers;
		expect(patchHeaders.get("authorization")).toBe("Bearer token-a");
	});

	it("sends typed show-scoped output-route intents", async () => {
		const outcome = {
			request_id: "ignored-by-client",
			replayed: false,
			change: {
				show_id: "show-a",
				show_revision: 2,
				route_id: "main",
				object_revision: 1,
				route: null,
				deleted: false,
			},
			event_sequence: 1,
		};
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockImplementation(async () =>
				new Response(JSON.stringify(outcome), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		const route = {
			protocol: "art_net" as const,
			logical_universe: 1,
			destination_universe: 2,
			delivery_mode: "broadcast" as const,
			destination: null,
			enabled: true,
			minimum_slots: 128,
		};

		await client.saveOutputRoute("show-a", "main", route, 0);
		await client.saveOutputRoute("show-a", "main", route, 3);
		await client.deleteOutputRoute("show-a", "main", 4);

		expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
			"http://desk.local/api/v2/output-routes/actions",
			"http://desk.local/api/v2/output-routes/actions",
			"http://desk.local/api/v2/output-routes/actions",
		]);
		const requests = fetchMock.mock.calls
			.slice(1)
			.map((call) => JSON.parse(call[1].body as string));
		expect(requests[0].action).toEqual({
			type: "create",
			route_id: "main",
			route,
		});
		expect(requests[1].action).toEqual({
			type: "update",
			route_id: "main",
			expected_revision: 3,
			patch: route,
		});
		expect(requests[2].action).toEqual({
			type: "delete",
			route_id: "main",
			expected_revision: 4,
		});
		for (const call of fetchMock.mock.calls.slice(1)) {
			const headers = call[1].headers as Headers;
			expect(headers.get("authorization")).toBe("Bearer token-a");
			expect(headers.get("x-tosk-show")).toBe("show-a");
		}
	});

	it("sends typed intents for layouts, patch layers, dynamics, and preload", async () => {
		const outcome = {
			request_id: "request",
			replayed: false,
			show_id: "show-a",
			show_revision: 2,
			object: {
				kind: "fixture",
				id: "object",
				revision: 1,
				updated_at: "2026-07-24T00:00:00Z",
				body: {},
			},
			event_sequence: 1,
		};
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockImplementation(async () =>
				new Response(JSON.stringify(outcome), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		await client.updateUserLayout(
			"show-a",
			"user-a",
			{ desks: [{ id: "main" }], activeDeskId: "main" },
			3,
		);
		await client.savePatchLayer(
			"show-a",
			{ id: "front", name: "Front truss", order: 2 },
			4,
		);
		await client.recordDynamic("show-a", "cue-list", 5, {
			speed: 60,
			width: 25,
			direction: "Reverse",
			fixtureIds: ["00000000-0000-4000-8000-000000000001"],
			groupIds: [],
		});
		await client.storePreload(
			"show-a",
			{
				target: "preset",
				target_id: "2.1",
				name: "Blue",
				mode: "overwrite",
				family: "Color",
			},
			6,
		);

		expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
			"http://desk.local/api/v2/user-layouts/user-a/update",
			"http://desk.local/api/v2/patch/layers/front/update",
			"http://desk.local/api/v2/cue-lists/cue-list/dynamics/record",
			"http://desk.local/api/v2/preload/record",
		]);
		const requests = fetchMock.mock.calls
			.slice(1)
			.map((call) => JSON.parse(call[1].body as string));
		expect(requests.map((request) => request.action)).toEqual([
			{
				type: "update",
				expected_revision: 3,
				patch: { desks: [{ id: "main" }], active_desk_id: "main" },
			},
			{
				type: "save",
				expected_revision: 4,
				layer: { name: "Front truss", order: 2 },
			},
			{
				type: "append",
				expected_revision: 5,
				speed: 60,
				width: 25,
				direction: "reverse",
				fixture_ids: ["00000000-0000-4000-8000-000000000001"],
				group_ids: [],
			},
			{
				type: "preset",
				target_id: "2.1",
				expected_revision: 6,
				name: "Blue",
				mode: "overwrite",
				family: "color",
			},
		]);
		for (const request of requests) {
			expect(request.request_id).toEqual(expect.any(String));
		}
		for (const call of fetchMock.mock.calls.slice(1)) {
			const headers = call[1].headers as Headers;
			expect(headers.get("authorization")).toBe("Bearer token-a");
			expect(headers.get("x-tosk-show")).toBe("show-a");
		}
	});

	it("reads one authenticated portable show object by its encoded identity", async () => {
		const stored = {
			kind: "user/layout",
			id: "operator one",
			revision: 8,
			updated_at: "2026-07-19T00:00:00Z",
			body: { desks: [] },
		};
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(JSON.stringify({
					show_id: "show one",
					show_revision: 8,
					kind: "user/layout",
					object_id: "operator one",
					object: stored,
				}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		await expect(
			client.object("show one", "user/layout", "operator one"),
		).resolves.toEqual(stored);

		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v2/objects/user%2Flayout/operator%20one",
		);
		const headers = fetchMock.mock.calls[1][1].headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer token-a");
		expect(headers.get("x-tosk-show")).toBe("show one");
	});

	it("returns authoritative absence only for a missing optional object", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						show_id: "show-a",
						show_revision: 8,
						kind: "group",
						object_id: "1",
						object: null,
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		await expect(
			client.objectOrNull("show-a", "group", "1"),
		).resolves.toBeNull();
	});

	it("does not hide failures while loading an optional object", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response("object service unavailable", { status: 503 }),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		await expect(
			client.objectOrNull("show-a", "group", "1"),
		).rejects.toMatchObject({
			message: "object service unavailable",
			status: 503,
		});
	});
});

describe("LightApiClient programmer and preset contracts", () => {
	it("authenticates the legacy Programmer compatibility projection", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(JSON.stringify([]), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");

		await client.login("Operator");
		await client.programmers();

		const headers = fetchMock.mock.calls[1][1].headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer token-a");
	});

	it("uses fixture-action and opt-in preset-generation commands", async () => {
		const client = new LightApiClient("http://desk.local");
		const command = vi
			.spyOn(client, "command")
			.mockResolvedValue({ created: [] });

		await client.controlFixtureAction("fixture-a", "action-a", true);
		await client.generateFixturePresets(["fixture-a"]);

		expect(command.mock.calls).toEqual([
			[
				"programmer.control_action",
				{
					fixture_id: "fixture-a",
					action_id: "action-a",
					active: true,
				},
			],
			["preset.generate_fixture_values", { fixture_ids: ["fixture-a"] }],
		]);
	});
});

describe("LightApiClient show lifecycle contracts", () => {
	it("creates named revisions, loads them as copies, and overwrites by stable IDs", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						request_id: "save-revision",
						replayed: false,
						result: {
							type: "revision",
							revision: {
								show_id: "show-a",
								revision: 1,
								name: "Before experiment",
								created_at: "2026-07-14T00:00:00Z",
							},
						},
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						request_id: "open-revision",
						replayed: false,
						result: {
							type: "show",
							show: {
								id: "copy-a",
								name: "Tour-rev-1-2026-07-17",
								revision_copy: { show_id: "show-a", revision: 1 },
							},
						},
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						request_id: "overwrite",
						replayed: false,
						result: {
							type: "show",
							show: { id: "show-a", name: "Tour" },
						},
					}),
					{
						status: 200,
						headers: { "content-type": "application/json" },
					},
				),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.saveShowRevision("show-a", "Before experiment");
		await client.openShowRevision("show-a", 1);
		await client.overwriteShow("copy-a", "show-a");
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v2/shows",
		);
		expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual(
			expect.objectContaining({
				request_id: expect.any(String),
				action: {
					type: "save_revision",
					show_id: "show-a",
					name: "Before experiment",
				},
			}),
		);
		expect(fetchMock.mock.calls[2][0]).toBe(
			"http://desk.local/api/v2/shows",
		);
		expect(fetchMock.mock.calls[3][0]).toBe(
			"http://desk.local/api/v2/shows",
		);
		expect(fetchMock.mock.calls[3][1].method).toBe("POST");
	});
	it("renames a show through its stable identity", async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						request_id: "rename",
						replayed: false,
						result: {
							type: "show",
							show: { id: "show-a", name: "Opening Night" },
						},
					}),
					{
						status: 200,
						headers: { "content-type": "application/json" },
					},
				),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.renameShow("show-a", "Opening Night");
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v2/shows",
		);
		expect(fetchMock.mock.calls[1][1].method).toBe("POST");
		expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual(
			expect.objectContaining({
				request_id: expect.any(String),
				action: {
					type: "rename",
					show_id: "show-a",
					name: "Opening Night",
				},
			}),
		);
	});
});

describe("LightApiClient authenticated desk services", () => {
	it("sends the optional desk boundary token before login", async () => {
		const fetchMock = vi.fn().mockResolvedValue(
			new Response(
				JSON.stringify({
					session_id: "session-a",
					token: "token-a",
					user: { id: "user-a", name: "Operator", enabled: true },
				}),
				{ status: 200, headers: { "content-type": "application/json" } },
			),
		);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		client.setDeskToken("desk secret");
		await client.login("Operator");
		expect(
			(fetchMock.mock.calls[0][1].headers as Headers).get("x-light-desk-token"),
		).toBe("desk secret");
	});

	it("uses the typed Sound-to-Light REST contract without sending a browser device ID", async () => {
		const configuration = {
			enabled: true,
			analysis_mode: "tempo_bpm" as const,
			frequency: { type: "preset" as const, preset: "low" as const },
			input_gain_db: 3,
			confidence_threshold: 0.7,
			smoothing: 0.25,
			minimum_bpm: 50,
			maximum_bpm: 200,
			signal_hold_millis: 1_500,
			multiplier: 2,
		};
		const state = {
			group: "A",
			configuration,
			snapshot: {
				manual_bpm: 100,
				sound_bpm: null,
				effective_bpm: 100,
				source: "manual",
				sound_status: { state: "disabled" },
				paused: false,
				phase_advancing: true,
				speed_master_scale: 1,
				sound_multiplier: 2,
				source_available: false,
				usable_signal: false,
				input_level: 0,
				selected_band_level: 0,
			},
		};
		const response = () =>
			new Response(JSON.stringify(state), {
				status: 200,
				headers: { "content-type": "application/json" },
			});
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
						desk: { id: "desk-a" },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockImplementation(() => Promise.resolve(response()));
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.speedGroup("A");
		await client.updateSpeedGroup("A", configuration);
		await client.observeSpeedGroup("A", {
			captured_at_millis: 100,
			source_available: true,
			usable_signal: true,
			level: 0.5,
			selected_band_level: 0.8,
			detected_bpm: 120,
			confidence: 0.9,
		});
		await client.speedGroupAction("A", {
			action: "learn",
			captured_at_millis: 101,
		});

		expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
			"http://desk.local/api/v1/speed-groups/A",
			"http://desk.local/api/v1/speed-groups/A",
			"http://desk.local/api/v1/speed-groups/A/observation",
			"http://desk.local/api/v1/speed-groups/A/action",
		]);
		expect(fetchMock.mock.calls[2][1].method).toBe("PUT");
		expect(JSON.parse(fetchMock.mock.calls[2][1].body)).toEqual(configuration);
		expect(JSON.parse(fetchMock.mock.calls[2][1].body)).not.toHaveProperty(
			"device_id",
		);
		expect(fetchMock.mock.calls[3][1].method).toBe("POST");
		expect(JSON.parse(fetchMock.mock.calls[3][1].body)).toMatchObject({
			detected_bpm: 120,
			confidence: 0.9,
		});
		expect(JSON.parse(fetchMock.mock.calls[4][1].body)).toEqual({
			action: "learn",
			captured_at_millis: 101,
		});
	});
});

describe("LightApiClient Highlight contracts", () => {
	it("reads and changes server-authoritative Highlight state", async () => {
		const state = {
			active: true,
			mode: "selection",
			output_enabled: true,
			capture_only: false,
			remembered: [{ fixture_id: "fixture-a", number: 1, name: "Spot" }],
			active_index: null,
			active_fixture: null,
			can_previous: false,
			can_next: true,
			owner_user_id: "user-a",
		};
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						session_id: "session-a",
						token: "token-a",
						user: { id: "user-a", name: "Operator", enabled: true },
						desk: { id: "desk-a" },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockImplementation(() =>
				Promise.resolve(
					new Response(JSON.stringify(state), {
						status: 200,
						headers: { "content-type": "application/json" },
					}),
				),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		expect(await client.highlight()).toEqual(state);
		expect(await client.highlightAction("all")).toEqual(state);
		await client.setPatchPreviewHighlight(true, ["fixture-a", "fixture-b"]);
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v1/highlight",
		);
		expect(fetchMock.mock.calls[2][0]).toBe(
			"http://desk.local/api/v1/highlight/action",
		);
		expect(fetchMock.mock.calls[2][1].method).toBe("POST");
		expect(JSON.parse(fetchMock.mock.calls[2][1].body)).toEqual({
			action: "all",
		});
		expect(
			(fetchMock.mock.calls[2][1].headers as Headers).get("authorization"),
		).toBe("Bearer token-a");
		expect(fetchMock.mock.calls[3][0]).toBe(
			"http://desk.local/api/v1/patch-preview-highlight",
		);
		expect(fetchMock.mock.calls[3][1].method).toBe("PUT");
		expect(JSON.parse(fetchMock.mock.calls[3][1].body)).toEqual({
			active: true,
			fixture_ids: ["fixture-a", "fixture-b"],
		});
	});
});

describe("LightApiClient fixture-profile contracts", () => {
	it("uses the desk-wide fixture-profile revision contract", async () => {
		const profile = {
			schema_version: 2 as const,
			id: "profile-a",
			revision: 3,
			manufacturer: "Acme",
			name: "Orbit",
			short_name: "Orbit",
			fixture_type: "wash",
			notes: "",
			photograph_asset: null,
			stage_icon_asset: null,
			model_asset: null,
			physical: {
				width_millimetres: null,
				height_millimetres: null,
				depth_millimetres: null,
				weight_kilograms: null,
				power_watts: null,
			},
			modes: [],
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			reserved_source: null,
		};
		const response = (body: unknown) =>
			new Response(JSON.stringify(body), {
				status: 200,
				headers: { "content-type": "application/json" },
			});
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				response({
					session_id: "session-a",
					token: "token-a",
					user: { id: "user-a", name: "Operator", enabled: true },
				}),
			)
			.mockResolvedValueOnce(
				response({ profiles: [profile] }),
			)
			.mockResolvedValueOnce(
				response({
					warnings: ["A legacy mode could not be migrated"],
				}),
			)
			.mockResolvedValueOnce(response({ profiles: [profile] }))
			.mockResolvedValueOnce(
				response({
					request_id: "save-profile",
					replayed: false,
					result: {
						type: "profile",
						profile_id: profile.id,
						revision: 4,
					},
				}),
			)
			.mockResolvedValueOnce(
				response({
					profiles: [{ ...profile, revision: 4 }],
				}),
			)
			.mockResolvedValueOnce(
				response({
					request_id: "attach-gdtf",
					replayed: false,
					result: {
						type: "gdtf_attached",
						profile_id: profile.id,
						revision: profile.revision,
					},
				}),
			)
			.mockResolvedValueOnce(
				response({
					request_id: "import-package",
					replayed: false,
					result: {
						type: "profile",
						profile_id: profile.id,
						revision: profile.revision,
					},
				}),
			)
			.mockResolvedValueOnce(
				response({ profiles: [profile] }),
			)
			.mockResolvedValueOnce(
				new Response(new Uint8Array([0x50, 0x4b]), {
					status: 200,
					headers: { "content-type": "application/vnd.tosklight.fixture+zip" },
				}),
			)
			.mockResolvedValueOnce(
				response({
					request_id: "delete-profile",
					replayed: false,
					result: {
						type: "deleted",
						resource: "profile",
						id: profile.id,
						revision: profile.revision,
					},
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");

		await client.fixtureProfiles();
		await client.fixtureProfileWarnings();
		await client.fixtureProfileRevisions(profile.id);
		await client.putFixtureProfile(profile, profile.revision);
		await client.putFixtureProfileSourceGdtf(
			profile.id,
			profile.revision,
			new Uint8Array([0x50, 0x4b]),
		);
		await client.importFixturePackage(new Uint8Array([0x50, 0x4b]));
		await client.exportFixturePackage(profile.id, profile.revision);
		await client.deleteFixtureProfile(profile.id, profile.revision);

		expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
			"http://desk.local/api/v2/fixture-library/profiles",
			"http://desk.local/api/v2/fixture-library/warnings",
			"http://desk.local/api/v2/fixture-library/profiles/profile-a/revisions",
			"http://desk.local/api/v2/fixture-library",
			"http://desk.local/api/v2/fixture-library/profiles",
			"http://desk.local/api/v2/fixture-library",
			"http://desk.local/api/v2/fixture-library",
			"http://desk.local/api/v2/fixture-library/profiles",
			"http://desk.local/api/v2/fixture-library/profiles/profile-a/revisions/3/package",
			"http://desk.local/api/v2/fixture-library",
		]);
		expect(fetchMock.mock.calls[4][1].method).toBe("POST");
		expect(fetchMock.mock.calls[6][1].method).toBe("POST");
		expect(
			(fetchMock.mock.calls[6][1].headers as Headers).get("content-type"),
		).toBe("application/json");
		expect(fetchMock.mock.calls[7][1].method).toBe("POST");
		expect(
			(fetchMock.mock.calls[7][1].headers as Headers).get("content-type"),
		).toBe("application/json");
		expect(
			(fetchMock.mock.calls[9][1].headers as Headers).get("authorization"),
		).toBe("Bearer token-a");
		expect(fetchMock.mock.calls[10][1].method).toBe("POST");
	});
});
