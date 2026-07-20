import { describe, expect, it, vi } from "vitest";
import { HttpShowObjectSnapshotTransport } from "./ShowObjectSnapshotTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function response(body: unknown, etag = '"7"') {
	return new Response(JSON.stringify(body), {
		status: 200,
		headers: { "content-type": "application/json", etag },
	});
}

function exactResponse(body: unknown, status = 200, showRevision = '"7"') {
	return new Response(JSON.stringify(body), {
		status,
		headers: {
			"content-type": "application/json",
			"x-light-show-revision": showRevision,
		},
	});
}

function group(kind = "group") {
	return {
		kind,
		id: "1",
		revision: 3,
		updated_at: "2026-07-20T00:00:00Z",
		body: { name: "Front", fixtures: ["fixture-1"] },
	};
}

describe("HttpShowObjectSnapshotTransport", () => {
	it("is dormant until collection hydration and returns the authoritative Show revision", async () => {
		const fetchImplementation = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) => response([group()]),
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
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v1/shows/${SHOW_ID}/objects/group`,
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
	});

	it("rejects a missing or malformed Show revision ETag", async () => {
		for (const etag of ["", "7", '"-1"', '"9007199254740992"']) {
			const transport = new HttpShowObjectSnapshotTransport({
				baseUrl: "http://desk",
				sessionToken: "token",
				fetch: vi.fn(async () => response([group()], etag)) as typeof fetch,
			});
			await expect(transport.collection(SHOW_ID, "group")).rejects.toThrow(
				"$.headers.etag",
			);
		}
	});

	it("rejects foreign kinds and non-array collection bodies", async () => {
		for (const body of [[group("preset")], { objects: [group()] }]) {
			const transport = new HttpShowObjectSnapshotTransport({
				baseUrl: "http://desk",
				sessionToken: "token",
				fetch: vi.fn(async () => response(body)) as typeof fetch,
			});
			await expect(transport.collection(SHOW_ID, "group")).rejects.toThrow();
		}
	});

	it("hydrates one exact object or authoritative absence with the Show revision", async () => {
		const fetchImplementation = vi
			.fn()
			.mockResolvedValueOnce(exactResponse(group()))
			.mockResolvedValueOnce(exactResponse({ error: "show object not found" }, 404));
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
			`http://desk/api/v1/shows/${SHOW_ID}/objects/group/1`,
		);
	});

	it("rejects malformed exact-object revision or identity authority", async () => {
		for (const responseValue of [
			exactResponse(group(), 200, "7"),
			exactResponse({ ...group(), id: "2" }),
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
