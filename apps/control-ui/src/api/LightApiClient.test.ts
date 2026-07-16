import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LightApiClient, defaultServerUrl } from "./LightApiClient";

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

describe("LightApiClient", () => {
  it("uses same-origin in a browser and the loopback daemon in Tauri", () => {
    expect(defaultServerUrl(new URL("http://desk.local/") as unknown as Location)).toBe("http://desk.local");
    expect(defaultServerUrl(new URL("tauri://localhost/") as unknown as Location)).toBe("http://127.0.0.1:5000");
  });

  it("uses a session-only desktop test server without replacing the saved operator server", () => {
    localStorage.setItem("light.server-url", "http://desk.local:5000");
    sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:64649");
    expect(defaultServerUrl(new URL("tauri://localhost/") as unknown as Location)).toBe("http://127.0.0.1:64649");
    expect(localStorage.getItem("light.server-url")).toBe("http://desk.local:5000");
  });

  it("keeps desktop test identity and desk state out of persistent storage", async () => {
    sessionStorage.setItem("light.test-server-url", "http://127.0.0.1:64649");
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true }, desk: { id: "desk-a" } }), { status: 200, headers: { "content-type": "application/json" } })));
    await new LightApiClient().login("Operator");
    expect(sessionStorage.getItem("light.client-id")).toEqual(expect.any(String));
    expect(sessionStorage.getItem("light.control-desk")).toBe("desk-a");
    expect(localStorage.getItem("light.client-id")).toBeNull();
    expect(localStorage.getItem("light.control-desk")).toBeNull();
  });

  it("creates a username session and authenticates subsequent REST requests", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        session_id: "session-a",
        token: "secret-token",
        user: { id: "user-a", name: "Operator", enabled: true },
      }), { status: 200, headers: { "content-type": "application/json" } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ cue_lists: [], active: [] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local");

    await client.login("Operator");
    await client.playbacks();

    expect(fetchMock.mock.calls[0][0]).toBe("http://desk.local/api/v1/sessions");
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual(expect.objectContaining({ username: "Operator", client_id: expect.any(String) }));
    const authenticatedHeaders = fetchMock.mock.calls[1][1].headers as Headers;
    expect(authenticatedHeaders.get("authorization")).toBe("Bearer secret-token");
  });

  it("uses revision headers for portable show objects", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true } }), { status: 200, headers: { "content-type": "application/json" } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ revision: 8 }), { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local");
    await client.login("Operator");
    await client.putObject("show-a", "user_layout", "user-a", { desks: [] }, 7);
    const headers = fetchMock.mock.calls[1][1].headers as Headers;
    expect(headers.get("if-match")).toBe("7");
    expect(headers.get("authorization")).toBe("Bearer token-a");
  });
  it("creates and restores named show revisions", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true } }), { status: 200, headers: { "content-type": "application/json" } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ show_id: "show-a", revision: 1, name: "Before experiment", created_at: "2026-07-14T00:00:00Z" }), { status: 201, headers: { "content-type": "application/json" } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ id: "show-a", name: "Tour" }), { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local");
    await client.login("Operator");
    await client.saveShowRevision("show-a", "Before experiment");
    await client.openShowRevision("show-a", 1);
    expect(fetchMock.mock.calls[1][0]).toBe("http://desk.local/api/v1/shows/show-a/revisions");
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({ name: "Before experiment" });
    expect(fetchMock.mock.calls[2][0]).toBe("http://desk.local/api/v1/shows/show-a/revisions/1/open");
  });
  it("sends the optional desk boundary token before login", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true } }), { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local"); client.setDeskToken("desk secret"); await client.login("Operator");
    expect((fetchMock.mock.calls[0][1].headers as Headers).get("x-light-desk-token")).toBe("desk secret");
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
    const response = () => new Response(JSON.stringify(state), { status: 200, headers: { "content-type": "application/json" } });
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true }, desk: { id: "desk-a" } }), { status: 200, headers: { "content-type": "application/json" } }))
      .mockImplementation(() => Promise.resolve(response()));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local");
    await client.login("Operator");
    await client.speedGroup("A");
    await client.updateSpeedGroup("A", configuration);
    await client.observeSpeedGroup("A", { captured_at_millis: 100, source_available: true, usable_signal: true, level: 0.5, selected_band_level: 0.8, detected_bpm: 120, confidence: 0.9 });
    await client.speedGroupAction("A", { action: "learn", captured_at_millis: 101 });

    expect(fetchMock.mock.calls.slice(1).map((call) => call[0])).toEqual([
      "http://desk.local/api/v1/speed-groups/A",
      "http://desk.local/api/v1/speed-groups/A",
      "http://desk.local/api/v1/speed-groups/A/observation",
      "http://desk.local/api/v1/speed-groups/A/action",
    ]);
    expect(fetchMock.mock.calls[2][1].method).toBe("PUT");
    expect(JSON.parse(fetchMock.mock.calls[2][1].body)).toEqual(configuration);
    expect(JSON.parse(fetchMock.mock.calls[2][1].body)).not.toHaveProperty("device_id");
    expect(fetchMock.mock.calls[3][1].method).toBe("POST");
    expect(JSON.parse(fetchMock.mock.calls[3][1].body)).toMatchObject({ detected_bpm: 120, confidence: 0.9 });
    expect(JSON.parse(fetchMock.mock.calls[4][1].body)).toEqual({ action: "learn", captured_at_millis: 101 });
  });
});
