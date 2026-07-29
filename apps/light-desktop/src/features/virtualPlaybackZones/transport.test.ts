import { describe, expect, it, vi } from "vitest";
import { VirtualPlaybackZonesProtocolError } from "./wire";
import {
	HttpVirtualPlaybackZonesTransport,
	VirtualPlaybackZonesHttpError,
} from "./transport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const OTHER_ID = "33333333-3333-4333-8333-333333333333";
const SCOPE = { showId: SHOW_ID, deskId: DESK_ID };
const ZONES = [{ id: "paired", name: "Paired", slots: [1, 2] }] as const;
const PAGE_MODE = { type: "pinned", page: 7 } as const;

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}

class FakeWebSocket extends EventTarget {
	static readonly OPEN = 1;
	readonly sent: string[] = [];
	readonly readyState = FakeWebSocket.OPEN;
	closed = false;

	constructor(
		readonly url: string | URL,
		readonly protocols?: string | string[],
	) {
		super();
	}

	send(value: string) {
		this.sent.push(value);
	}

	close() {
		this.closed = true;
		this.dispatchEvent(new Event("close"));
	}
}

function createTransport(
	fetchImplementation: typeof globalThis.fetch,
	webSocket?: typeof globalThis.WebSocket,
) {
	return new HttpVirtualPlaybackZonesTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch: fetchImplementation,
		webSocket,
	});
}

describe("HttpVirtualPlaybackZonesTransport", () => {
	it("is dormant until a caller explicitly loads", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		const transport = createTransport(fetchImplementation);

		expect(fetchImplementation).not.toHaveBeenCalled();
		fetchImplementation.mockResolvedValueOnce(
			json({ show_id: SHOW_ID, desks: { [DESK_ID]: {} } }),
		);
		await expect(transport.loadSnapshot(SCOPE)).resolves.toMatchObject({
			showId: SHOW_ID,
			desks: { [DESK_ID]: {} },
		});

		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			"http://127.0.0.1:5000/api/v2/virtual-playback-exclusion-zones",
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
		expect(headers.get("x-tosk-show")).toBe(SHOW_ID);
		expect(headers.get("x-tosk-desk")).toBe(DESK_ID);
	});

	it("saves one exact surface through an authenticated PUT", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation.mockResolvedValueOnce(
			json({
				request_id: "request-a",
				show_id: SHOW_ID,
				desk_id: DESK_ID,
				surface_id: "surface/one",
				surface: {
					revision: 5,
					page_mode: PAGE_MODE,
					zones: ZONES,
				},
				replayed: false,
				changed: true,
			}),
		);
		const transport = createTransport(fetchImplementation);

		await expect(
			transport.saveSurface(
				SCOPE,
				"surface/one",
				4,
				PAGE_MODE,
				ZONES,
				"request-a",
			),
		).resolves.toMatchObject({
			surfaceId: "surface/one",
			surface: { revision: 5, pageMode: PAGE_MODE, zones: ZONES },
		});
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			"http://127.0.0.1:5000/api/v2/virtual-playback-exclusion-zones/surface%2Fone/update",
		);
		expect(init?.method).toBe("POST");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			page_mode: PAGE_MODE,
			zones: ZONES,
		});
		expect(new Headers(init?.headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe(SHOW_ID);
		expect(new Headers(init?.headers).get("x-tosk-desk")).toBe(DESK_ID);
	});

	it("rejects foreign or malformed successful responses", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation
			.mockResolvedValueOnce(
					json({ show_id: OTHER_ID, desks: { [DESK_ID]: {} } }),
			)
			.mockResolvedValueOnce(
					json({
						request_id: "request-a",
						show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "foreign",
					surface: {
						revision: 5,
						page_mode: PAGE_MODE,
						zones: ZONES,
					},
						replayed: false,
						changed: true,
				}),
			)
			.mockResolvedValueOnce(
					json({
						request_id: "request-a",
						show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "surface-a",
					surface: {
						revision: 5,
						page_mode: PAGE_MODE,
						zones: [{ ...ZONES[0], slots: [1, 8_999] }],
					},
						replayed: false,
						changed: true,
				}),
			);
		const transport = createTransport(fetchImplementation);

		await expect(transport.loadSnapshot(SCOPE)).rejects.toBeInstanceOf(
			VirtualPlaybackZonesProtocolError,
		);
		await expect(
			transport.saveSurface(SCOPE, "surface-a", 4, PAGE_MODE, ZONES, "request-a"),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
		await expect(
			transport.saveSurface(SCOPE, "surface-a", 4, PAGE_MODE, ZONES, "request-a"),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
	});

	it("rejects a stale save outcome after captured scope is replaced", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation.mockResolvedValueOnce(
			json({
				request_id: "request-a",
				show_id: OTHER_ID,
				desk_id: DESK_ID,
				surface_id: "surface-a",
				surface: {
					revision: 5,
					page_mode: PAGE_MODE,
					zones: ZONES,
				},
				replayed: false,
				changed: true,
			}),
		);
		const transport = createTransport(fetchImplementation);

		await expect(
			transport.saveSurface(SCOPE, "surface-a", 4, PAGE_MODE, ZONES, "request-a"),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).not.toContain(`/shows/`);
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe(SHOW_ID);
	});

	it("reports an HTTP error without accepting its payload", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation.mockResolvedValueOnce(json({ error: "denied" }, 403));
		const transport = createTransport(fetchImplementation);

		await expect(transport.loadSnapshot(SCOPE)).rejects.toMatchObject({
			name: "VirtualPlaybackZonesHttpError",
			message: "denied",
			status: 403,
		} satisfies Partial<VirtualPlaybackZonesHttpError>);
	});

	it("subscribes to the show projection and decodes zone invalidations", () => {
		const sockets: FakeWebSocket[] = [];
		class CapturingWebSocket extends FakeWebSocket {
			constructor(url: string | URL, protocols?: string | string[]) {
				super(url, protocols);
				sockets.push(this);
			}
		}
		const transport = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			CapturingWebSocket as unknown as typeof WebSocket,
		);
		const observer = {
			changed: vi.fn(),
			gap: vi.fn(),
			error: vi.fn(),
			closed: vi.fn(),
		};
		const stream = transport.subscribe(SCOPE, observer);
		sockets[0].dispatchEvent(new Event("open"));

		expect(String(sockets[0].url)).toBe("ws://127.0.0.1:5000/api/v2/events");
		expect(JSON.parse(sockets[0].sent[0])).toMatchObject({
			type: "subscribe",
			filter: {
				capabilities: ["show"],
				classes: ["projection"],
				objects: [
					{
						capability: "show",
						id: `virtual-playback-exclusion-zones:${SHOW_ID}`,
					},
				],
			},
		});
		sockets[0].dispatchEvent(
			new MessageEvent("message", {
				data: JSON.stringify({
					type: "event",
					event: {
						payload: {
							type: "virtual_playback_exclusion_zones_changed",
							change: {
								show_id: SHOW_ID,
								desk_id: DESK_ID,
								surface_id: "surface-a",
							},
						},
					},
				}),
			}),
		);
		expect(observer.changed).toHaveBeenCalledWith({
			showId: SHOW_ID,
			deskId: DESK_ID,
			surfaceId: "surface-a",
		});

		stream.close();
		expect(sockets[0].closed).toBe(true);
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
