import { describe, expect, it, vi } from "vitest";
import { HttpShowObjectSnapshotTransport } from "./ShowObjectSnapshotTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function group(kind = "group") {
	return {
		kind,
		id: "1",
		revision: 3,
		updated_at: "2026-07-20T00:00:00Z",
		body: { name: "Front", fixtures: ["fixture-1"] },
		validationError: null,
	};
}

function collectionResponse(
	objects: unknown = [group()],
	showRevision: unknown = 7,
	kind = "group",
) {
	return new Response(
		JSON.stringify({
			show_id: SHOW_ID,
			show_revision: showRevision,
			kind,
			objects,
		}),
		{ status: 200, headers: { "content-type": "application/json" } },
	);
}

function exactResponse(
	object: unknown = group(),
	showRevision: unknown = 7,
	objectId = "1",
) {
	return new Response(
		JSON.stringify({
			show_id: SHOW_ID,
			show_revision: showRevision,
			kind: "group",
			object_id: objectId,
			object,
		}),
		{ status: 200, headers: { "content-type": "application/json" } },
	);
}

describe("HttpShowObjectSnapshotTransport", () => {
	it("is dormant until collection hydration and returns the authoritative Show revision", async () => {
		const fetchImplementation = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				collectionResponse(),
		);
		const transport = new HttpShowObjectSnapshotTransport({
			baseUrl: "http://127.0.0.1:5000/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch: fetchImplementation as typeof fetch,
		});
		expect(fetchImplementation).not.toHaveBeenCalled();

		await expect(transport.collection(SHOW_ID, "group")).resolves.toEqual({
			objects: [group()],
			showRevision: 7,
		});
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe("http://127.0.0.1:5000/api/v2/objects/group");
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(headers.get("x-tosk-show")).toBe(SHOW_ID);
	});

	it("rejects malformed snapshot revision or identity authority", async () => {
		for (const responseValue of [
			collectionResponse([], -1),
			collectionResponse([], Number.MAX_SAFE_INTEGER + 1),
			collectionResponse([], 7, "preset"),
			new Response(
				JSON.stringify({
					show_id: "22222222-2222-4222-8222-222222222222",
					show_revision: 7,
					kind: "group",
					objects: [],
				}),
			),
		]) {
			const transport = new HttpShowObjectSnapshotTransport({
				baseUrl: "http://desk",
				sessionToken: "token",
				fetch: vi.fn(async () => responseValue) as typeof fetch,
			});
			await expect(transport.collection(SHOW_ID, "group")).rejects.toThrow();
		}
	});

	it("rejects foreign kinds and non-array collection bodies", async () => {
		for (const responseValue of [
			collectionResponse([group("preset")]),
			collectionResponse({ objects: [group()] }),
		]) {
			const transport = new HttpShowObjectSnapshotTransport({
				baseUrl: "http://desk",
				sessionToken: "token",
				fetch: vi.fn(async () => responseValue) as typeof fetch,
			});
			await expect(transport.collection(SHOW_ID, "group")).rejects.toThrow();
		}
	});

	it("hydrates one exact object or authoritative absence with the Show revision", async () => {
		const fetchImplementation = vi
			.fn()
			.mockResolvedValueOnce(exactResponse())
			.mockResolvedValueOnce(exactResponse(null, 7, "missing"));
		const transport = new HttpShowObjectSnapshotTransport({
			baseUrl: "http://desk",
			sessionToken: "token",
			fetch: fetchImplementation as typeof fetch,
		});

		await expect(transport.object(SHOW_ID, "group", "1")).resolves.toEqual({
			object: group(),
			showRevision: 7,
		});
		await expect(transport.object(SHOW_ID, "group", "missing")).resolves.toEqual({
			object: null,
			showRevision: 7,
		});
		expect(fetchImplementation.mock.calls[0][0]).toBe(
			"http://desk/api/v2/objects/group/1",
		);
	});

	it("rejects malformed exact-object revision or identity authority", async () => {
		for (const responseValue of [
			exactResponse(group(), "7"),
			exactResponse({ ...group(), id: "2" }),
			exactResponse(group(), 7, "2"),
		]) {
			const transport = new HttpShowObjectSnapshotTransport({
				baseUrl: "http://desk",
				sessionToken: "token",
				fetch: vi.fn(async () => responseValue) as typeof fetch,
			});
			await expect(transport.object(SHOW_ID, "group", "1")).rejects.toThrow();
		}
	});
});
