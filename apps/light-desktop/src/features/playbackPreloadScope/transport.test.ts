import { describe, it, expect, vi } from "vitest";
import { HttpPreloadScopeTransport } from "./transport";
const SHOW = "11111111-1111-4111-8111-111111111111";
const authority = { showId: SHOW };
const scope = { position_ranges: [{ start: 1, end: 1000 }], pages: null };
function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}
function transport(fetch: typeof globalThis.fetch) {
	return new HttpPreloadScopeTransport({
		baseUrl: "http://127.0.0.1:5000",
		sessionToken: "test-token",
		fetch,
	});
}
describe("Preload scope authenticated transport", () => {
	it("loads only the show-owned eligibility snapshot", async () => {
		const request = vi
			.fn<typeof fetch>()
			.mockResolvedValue(json({ show_id: SHOW, revision: 4, scope }));
		expect(await transport(request).loadSnapshot(authority)).toEqual({
			show_id: SHOW,
			revision: 4,
			scope,
		});
		expect(request.mock.calls[0][0]).toBe(
			"http://127.0.0.1:5000/api/v2/playback-preload-scope",
		);
		expect(
			new Headers(request.mock.calls[0][1]?.headers).get("x-tosk-show"),
		).toBe(SHOW);
	});
	it("sends one guarded intent and validates request correlation", async () => {
		const request = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(
				json({
					show_id: SHOW,
					revision: 5,
					scope,
					request_id: "save1",
					replayed: false,
					changed: true,
				}),
			)
			.mockResolvedValueOnce(
				json({
					show_id: SHOW,
					revision: 5,
					scope,
					request_id: "other",
					replayed: true,
					changed: false,
				}),
			);
		await transport(request).save(authority, 4, scope, "save1");
		expect(JSON.parse(String(request.mock.calls[0][1]?.body))).toEqual({
			request_id: "save1",
			expected_revision: 4,
			scope,
		});
		expect(request.mock.calls[0][0]).toBe(
			"http://127.0.0.1:5000/api/v2/playback-preload-scope/update",
		);
		await expect(
			transport(request).save(authority, 4, scope, "save1"),
		).rejects.toThrow("request_id");
	});
	it("reports conflict without automatic overwrite", async () => {
		const request = vi
			.fn<typeof fetch>()
			.mockResolvedValue(json({ error: "stale revision" }, 409));
		await expect(
			transport(request).save(authority, 4, scope, "save1"),
		).rejects.toThrow("stale revision");
		expect(request).toHaveBeenCalledOnce();
	});
	it("subscribes exact shared object identity and closes callbacks", () => {
		let socket!: FakeSocket;
		class FakeSocket extends EventTarget {
			send = vi.fn();
			close = vi.fn();
			constructor() {
				super();
				socket = this;
			}
		}
		const observer = {
			changed: vi.fn(),
			gap: vi.fn(),
			error: vi.fn(),
			closed: vi.fn(),
			ready: vi.fn(),
		};
		const adapter = new HttpPreloadScopeTransport({
			baseUrl: "http://127.0.0.1:5000",
			sessionToken: "test-token",
			webSocket: FakeSocket as unknown as typeof WebSocket,
		});
		const stream = adapter.subscribe(authority, observer);
		socket.dispatchEvent(new Event("open"));
		expect(JSON.parse(socket.send.mock.calls[0][0]).filter.objects).toEqual([
			{ capability: "show", id: `playback-preload-scope:${SHOW}` },
		]);
		socket.dispatchEvent(
			new MessageEvent("message", {
				data: JSON.stringify({
					type: "event",
					event: {
						payload: {
							type: "playback_preload_scope_changed",
							change: { show_id: SHOW, revision: 5 },
						},
					},
				}),
			}),
		);
		expect(observer.changed).toHaveBeenCalledWith({
			showId: SHOW,
			revision: 5,
		});
		stream.close();
		socket.dispatchEvent(new MessageEvent("message", { data: "invalid" }));
		expect(observer.error).not.toHaveBeenCalled();
		expect(socket.close).toHaveBeenCalledOnce();
	});
});
