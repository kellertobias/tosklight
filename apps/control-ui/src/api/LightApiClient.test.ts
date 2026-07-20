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
				new Response(JSON.stringify({ cue_lists: [], active: [] }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");

		await client.login("Operator");
		await client.playbacks();

		expect(fetchMock.mock.calls[0][0]).toBe(
			"http://desk.local/api/v1/sessions",
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

	it("uses revision headers for portable show objects", async () => {
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
				new Response(JSON.stringify({ revision: 8 }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.putObject("show-a", "user_layout", "user-a", { desks: [] }, 7);
		const headers = fetchMock.mock.calls[1][1].headers as Headers;
		expect(headers.get("if-match")).toBe("7");
		expect(headers.get("authorization")).toBe("Bearer token-a");
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
				new Response(JSON.stringify(stored), {
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
			"http://desk.local/api/v1/shows/show%20one/objects/user%2Flayout/operator%20one",
		);
		const headers = fetchMock.mock.calls[1][1].headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer token-a");
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
			.mockResolvedValueOnce(new Response("missing", { status: 404 }));
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
	it("addresses presets by family and pool-local number", async () => {
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
				new Response(JSON.stringify({ revision: 1 }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		const command = vi.spyOn(client, "command").mockResolvedValue({});

		await client.applyPreset({ family: "Color", number: 1 });
		await client.storePreset(
			"show-a",
			{ family: "Position", number: 1 },
			{
				name: "Position one",
				family: "Position",
				number: 1,
				values: {},
				group_values: {},
			},
			"overwrite",
			0,
		);

		expect(command).toHaveBeenCalledWith("preset.apply", {
			family: "Color",
			number: 1,
		});
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v1/shows/show-a/presets/3.1/store",
		);
		expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toMatchObject({
			preset: { family: "Position", number: 1 },
		});
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
						show_id: "show-a",
						revision: 1,
						name: "Before experiment",
						created_at: "2026-07-14T00:00:00Z",
					}),
					{ status: 201, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						id: "copy-a",
						name: "Tour-rev-1-2026-07-17",
						revision_copy: { show_id: "show-a", revision: 1 },
					}),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
			)
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ id: "show-a", name: "Tour" }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.saveShowRevision("show-a", "Before experiment");
		await client.openShowRevision("show-a", 1);
		await client.overwriteShow("copy-a", "show-a");
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v1/shows/show-a/revisions",
		);
		expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
			name: "Before experiment",
		});
		expect(fetchMock.mock.calls[2][0]).toBe(
			"http://desk.local/api/v1/shows/show-a/revisions/1/open",
		);
		expect(fetchMock.mock.calls[3][0]).toBe(
			"http://desk.local/api/v1/shows/copy-a/overwrite/show-a",
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
				new Response(JSON.stringify({ id: "show-a", name: "Opening Night" }), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			);
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.renameShow("show-a", "Opening Night");
		expect(fetchMock.mock.calls[1][0]).toBe(
			"http://desk.local/api/v1/shows/show-a/rename",
		);
		expect(fetchMock.mock.calls[1][1].method).toBe("PUT");
		expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
			name: "Opening Night",
		});
	});
});

describe("LightApiClient Update contracts", () => {
	it("uses the desk-scoped Update settings, preview, apply, and eligible-target contracts", async () => {
		const settings = {
			cue_mode: "add_to_current_cue" as const,
			preset_mode: "update_existing" as const,
			group_mode: "update_existing" as const,
			other_target_modes: {},
			show_update_modal_on_touch: true,
		};
		const target = {
			family: { type: "cue" as const },
			object_id: "cue-list-a",
			playback_number: 7,
			cue_id: "cue-a",
			cue_number: 2,
		};
		const mode = {
			target_type: "cue" as const,
			mode: "existing_only" as const,
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
					desk: { id: "desk-a" },
				}),
			)
			.mockResolvedValueOnce(response(settings))
			.mockResolvedValueOnce(response(settings))
			.mockResolvedValueOnce(
				response({
					revision: 4,
					programmer_revision: "programmer-a",
					target: {
						...target,
						name: "Main",
						family: target.family,
						cue: { id: "cue-a", number: 2 },
					},
					mode,
					items: [],
				}),
			)
			.mockResolvedValueOnce(
				response({
					target: { ...target, name: "Main", family: target.family },
					revision_before: 4,
					revision_after: 5,
					eligible_count: 1,
					changed_count: 1,
					added_count: 0,
					ignored_count: 0,
					changed_cues: [],
					programmer_values_retained: true,
				}),
			)
			.mockResolvedValueOnce(response([]));
		vi.stubGlobal("fetch", fetchMock);
		const client = new LightApiClient("http://desk.local");
		await client.login("Operator");
		await client.updateSettings();
		await client.saveUpdateSettings(settings);
		await client.previewUpdate(target, mode);
		await client.applyUpdate(target, mode, 4, "programmer-a");
		await client.updateTargets("show_all_active");

		expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
			"http://desk.local/api/v1/update/settings",
			"http://desk.local/api/v1/update/settings",
			"http://desk.local/api/v1/update/preview",
			"http://desk.local/api/v1/update/apply",
			"http://desk.local/api/v1/update/targets?filter=show_all_active",
		]);
		expect(fetchMock.mock.calls[2][1].method).toBe("PUT");
		expect(JSON.parse(fetchMock.mock.calls[3][1].body)).toEqual({
			target,
			mode,
		});
		expect(JSON.parse(fetchMock.mock.calls[4][1].body)).toEqual({
			target,
			mode,
			expected_revision: 4,
			expected_programmer_revision: "programmer-a",
		});
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
			.mockResolvedValueOnce(response([profile]))
			.mockResolvedValueOnce(response(["A legacy mode could not be migrated"]))
			.mockResolvedValueOnce(response([profile]))
			.mockResolvedValueOnce(response({ ...profile, revision: 4 }))
			.mockResolvedValueOnce(new Response(null, { status: 204 }))
			.mockResolvedValueOnce(response(profile))
			.mockResolvedValueOnce(
				new Response(new Uint8Array([0x50, 0x4b]), {
					status: 200,
					headers: { "content-type": "application/vnd.tosklight.fixture+zip" },
				}),
			)
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
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
			"http://desk.local/api/v1/fixture-profiles",
			"http://desk.local/api/v1/fixture-profiles/warnings",
			"http://desk.local/api/v1/fixture-profiles/profile-a/revisions",
			"http://desk.local/api/v1/fixture-profiles",
			"http://desk.local/api/v1/fixture-profiles/profile-a/3/source-gdtf",
			"http://desk.local/api/v1/fixture-packages/import",
			"http://desk.local/api/v1/fixture-profiles/profile-a/3/package",
			"http://desk.local/api/v1/fixture-profiles/profile-a/3",
		]);
		expect(fetchMock.mock.calls[4][1].method).toBe("PUT");
		expect(
			(fetchMock.mock.calls[4][1].headers as Headers).get("if-match"),
		).toBe("3");
		expect(fetchMock.mock.calls[5][1].method).toBe("PUT");
		expect(
			(fetchMock.mock.calls[5][1].headers as Headers).get("content-type"),
		).toBe("application/octet-stream");
		expect(fetchMock.mock.calls[6][1].method).toBe("POST");
		expect(
			(fetchMock.mock.calls[6][1].headers as Headers).get("content-type"),
		).toBe("application/vnd.tosklight.fixture+zip");
		expect(
			(fetchMock.mock.calls[7][1].headers as Headers).get("authorization"),
		).toBe("Bearer token-a");
		expect(fetchMock.mock.calls[8][1].method).toBe("DELETE");
	});
});
