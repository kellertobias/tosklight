import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LightApiClient, defaultServerUrl } from "./LightApiClient";

beforeEach(() => {
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
    clear: () => values.clear(),
  });
});
afterEach(() => vi.unstubAllGlobals());

describe("LightApiClient", () => {
  it("uses same-origin in a browser and the loopback daemon in Tauri", () => {
    expect(defaultServerUrl(new URL("http://desk.local/") as unknown as Location)).toBe("http://desk.local");
    expect(defaultServerUrl(new URL("tauri://localhost/") as unknown as Location)).toBe("http://127.0.0.1:5000");
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
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({ username: "Operator" });
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
  it("sends the optional desk boundary token before login", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ session_id: "session-a", token: "token-a", user: { id: "user-a", name: "Operator", enabled: true } }), { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetchMock);
    const client = new LightApiClient("http://desk.local"); client.setDeskToken("desk secret"); await client.login("Operator");
    expect((fetchMock.mock.calls[0][1].headers as Headers).get("x-light-desk-token")).toBe("desk secret");
  });
});
