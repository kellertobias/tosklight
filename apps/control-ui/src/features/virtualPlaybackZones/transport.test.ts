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

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}

function createTransport(fetchImplementation: typeof globalThis.fetch) {
	return new HttpVirtualPlaybackZonesTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch: fetchImplementation,
	});
}

describe("HttpVirtualPlaybackZonesTransport", () => {
	it("is dormant until a caller explicitly loads", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		const transport = createTransport(fetchImplementation);

		expect(fetchImplementation).not.toHaveBeenCalled();
		fetchImplementation.mockResolvedValueOnce(
			json({ show_id: SHOW_ID, desk_id: DESK_ID, surfaces: {} }),
		);
		await expect(transport.loadSnapshot(SCOPE)).resolves.toMatchObject({
			showId: SHOW_ID,
			deskId: DESK_ID,
		});

		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/shows/${SHOW_ID}/desks/${DESK_ID}/virtual-playback-exclusion-zones`,
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("saves one exact surface through an authenticated PUT", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation.mockResolvedValueOnce(
			json({
				show_id: SHOW_ID,
				desk_id: DESK_ID,
				surface_id: "surface/one",
				zones: ZONES,
			}),
		);
		const transport = createTransport(fetchImplementation);

		await expect(
			transport.saveSurface(SCOPE, "surface/one", ZONES),
		).resolves.toEqual({ surfaceId: "surface/one", zones: ZONES });
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/shows/${SHOW_ID}/desks/${DESK_ID}/virtual-playback-exclusion-zones/surface%2Fone`,
		);
		expect(init?.method).toBe("PUT");
		expect(JSON.parse(String(init?.body))).toEqual({ zones: ZONES });
		expect(new Headers(init?.headers).get("authorization")).toBe(
			"Bearer session-token",
		);
	});

	it("rejects foreign or malformed successful responses", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation
			.mockResolvedValueOnce(
				json({ show_id: OTHER_ID, desk_id: DESK_ID, surfaces: {} }),
			)
			.mockResolvedValueOnce(
				json({
					show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "foreign",
					zones: ZONES,
				}),
			)
			.mockResolvedValueOnce(
				json({
					show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "surface-a",
					zones: [{ ...ZONES[0], slots: [1, 145] }],
				}),
			);
		const transport = createTransport(fetchImplementation);

		await expect(transport.loadSnapshot(SCOPE)).rejects.toBeInstanceOf(
			VirtualPlaybackZonesProtocolError,
		);
		await expect(
			transport.saveSurface(SCOPE, "surface-a", ZONES),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
		await expect(
			transport.saveSurface(SCOPE, "surface-a", ZONES),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
	});

	it("rejects a stale save outcome after captured scope is replaced", async () => {
		const fetchImplementation = vi.fn<typeof globalThis.fetch>();
		fetchImplementation.mockResolvedValueOnce(
			json({
				show_id: OTHER_ID,
				desk_id: DESK_ID,
				surface_id: "surface-a",
				zones: ZONES,
			}),
		);
		const transport = createTransport(fetchImplementation);

		await expect(
			transport.saveSurface(SCOPE, "surface-a", ZONES),
		).rejects.toBeInstanceOf(VirtualPlaybackZonesProtocolError);
		const [url] = fetchImplementation.mock.calls[0];
		expect(url).toContain(`/shows/${SHOW_ID}/desks/${DESK_ID}/`);
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
});
