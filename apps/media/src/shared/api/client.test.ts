import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiFailure, api } from "./client";

afterEach(() => vi.unstubAllGlobals());

function answering(body: unknown, status: number): void {
	vi.stubGlobal(
		"fetch",
		vi.fn(
			async () =>
				new Response(JSON.stringify(body), {
					status,
					headers: { "content-type": "application/json" },
				}),
		),
	);
}

describe("the transport", () => {
	it("carries the server's stable code, not its wording", async () => {
		answering(
			{ code: "dmx-owns-this", message: "a desk is driving this output" },
			409,
		);

		const failure = await api.outputs().catch((error: unknown) => error);
		expect(failure).toBeInstanceOf(ApiFailure);
		expect((failure as ApiFailure).code).toBe("dmx-owns-this");
		expect((failure as ApiFailure).deskOwnsIt).toBe(true);
		expect((failure as ApiFailure).disconnected).toBe(false);
	});

	it("distinguishes a server that says no from one that is not there", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(async () => {
				throw new TypeError("network");
			}),
		);

		const failure = (await api
			.health()
			.catch((error: unknown) => error)) as ApiFailure;
		expect(failure.disconnected).toBe(true);
		expect(failure.code).toBe("unreachable");
	});

	it("still produces a readable failure when something answers that is not the API", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn(
				async () => new Response("<html>proxy error</html>", { status: 502 }),
			),
		);

		const failure = (await api
			.catalog()
			.catch((error: unknown) => error)) as ApiFailure;
		expect(failure.code).toBe("unexpected-response");
		expect(failure.status).toBe(502);
	});

	it("sends an intent-shaped body carrying only what changed", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(JSON.stringify({}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLayer("an-output", 2, { dimmer: 0.5 });

		const [url, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(url).toBe("/api/v2/outputs/an-output/layers/2/update");
		expect(init.body).toBe('{"dimmer":0.5}');
	});

	it("rounds and bounds every u8 layer field without changing fractional volume", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(JSON.stringify({}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLayer("an-output", 2, {
			folder: -3.2,
			file: 999.8,
			playModeDmx: 215.6,
			speedMultiplierDmx: 127.5,
			playbackBpm: 120.1,
			effectSlot: 2.8,
			volume: 0.375,
		});

		const [, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(init.body).toBe(
			'{"folder":0,"file":255,"playModeDmx":216,"speedMultiplierDmx":128,"playbackBpm":120,"effectSlot":3,"volume":0.375}',
		);
	});

	it("refuses non-finite u8 values before fetch", async () => {
		const fetchStub = vi.fn();
		vi.stubGlobal("fetch", fetchStub);

		let failure: unknown;
		try {
			api.updateLayer("an-output", 0, { playbackBpm: Number.NaN });
		} catch (error) {
			failure = error;
		}

		expect(failure).toBeInstanceOf(ApiFailure);
		expect((failure as ApiFailure).code).toBe("invalid-u8-control");
		expect(fetchStub).not.toHaveBeenCalled();
	});

	it("bounds u8 master mask addresses", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(JSON.stringify({}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		vi.stubGlobal("fetch", fetchStub);
		await api.updateMaster("an-output", {
			maskFolder: -1,
			maskFile: 300,
			volume: 0.42,
		});
		const [, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(init.body).toBe('{"maskFolder":0,"maskFile":255,"volume":0.42}');
	});

	it("sends a typed Analog TV parameter without replacing the effect chain", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(JSON.stringify({}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLayer("an-output", 2, {
			effectSlot: 1,
			imageGrain: 0.65,
		});

		const [, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(init.body).toBe('{"effectSlot":1,"imageGrain":0.65}');
	});

	it("sends one typed Digital TV parameter without replacing the effect chain", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(JSON.stringify({}), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLayer("an-output", 2, {
			effectSlot: 3,
			tileDisplacement: 0.7,
		});

		const [, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(init.body).toBe('{"effectSlot":3,"tileDisplacement":0.7}');
	});

	it("sends a folder reorder as an explicit swap intent", async () => {
		const fetchStub = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLibraryFolder(1, {
			requestId: "swap-folder",
			swapWith: 900,
		});

		const [url, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(url).toBe("/api/v2/library/folders/1/update");
		expect(init.method).toBe("POST");
		expect(init.body).toBe('{"requestId":"swap-folder","swapWith":900}');
	});

	it("sends folder compaction as one replay-safe server intent", async () => {
		const fetchStub = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLibraryFolder(7, {
			requestId: "compact-folder",
			compact: true,
		});

		const [url, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(url).toBe("/api/v2/library/folders/7/update");
		expect(init.method).toBe("POST");
		expect(init.body).toBe('{"requestId":"compact-folder","compact":true}');
	});

	it("sends one replay-safe note intent for selected media and folders", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(
					JSON.stringify({ revision: 1, itemCount: 0, folders: [] }),
					{
						status: 200,
						headers: { "content-type": "application/json" },
					},
				),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLibraryNotes({
			requestId: "notes",
			targets: [
				{ kind: "item", id: "asset-a" },
				{ kind: "folder", folder: 7 },
			],
			note: "Licence: CC BY 4.0",
		});

		const [url, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		expect(url).toBe("/api/v2/library/notes/update");
		expect(init.method).toBe("POST");
		expect(JSON.parse(String(init.body))).toEqual({
			requestId: "notes",
			targets: [
				{ kind: "item", id: "asset-a" },
				{ kind: "folder", folder: 7 },
			],
			note: "Licence: CC BY 4.0",
		});
	});

	it("sends replay-safe enable and delete intents for one media file", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(
					JSON.stringify({ revision: 1, itemCount: 0, folders: [] }),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLibraryItem("asset/a", {
			requestId: "disable",
			enabled: false,
			swap: false,
		});
		await api.deleteLibraryItem("asset/a", { requestId: "delete" });

		const [enableUrl, enableInit] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit,
		];
		const [deleteUrl, deleteInit] = fetchStub.mock.calls[1] as unknown as [
			string,
			RequestInit,
		];
		expect(enableUrl).toBe("/api/v2/library/items/asset%2Fa/update");
		expect(JSON.parse(String(enableInit.body))).toEqual({
			requestId: "disable",
			enabled: false,
			swap: false,
		});
		expect(deleteUrl).toBe("/api/v2/library/items/asset%2Fa/delete");
		expect(JSON.parse(String(deleteInit.body))).toEqual({
			requestId: "delete",
		});
	});

	it("sends one replay-safe intent for bulk enable and bulk delete", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(
					JSON.stringify({ revision: 1, itemCount: 0, folders: [] }),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.updateLibraryItems({
			requestId: "bulk-disable",
			ids: ["asset-a", "asset-b"],
			enabled: false,
		});
		await api.deleteLibraryItems({
			requestId: "bulk-delete",
			ids: ["asset-a", "asset-b"],
		});

		const calls = fetchStub.mock.calls as unknown as [string, RequestInit][];
		expect(calls.map(([url]) => url)).toEqual([
			"/api/v2/library/items/update",
			"/api/v2/library/items/delete",
		]);
		expect(calls.map(([, init]) => JSON.parse(String(init.body)))).toEqual([
			{
				requestId: "bulk-disable",
				ids: ["asset-a", "asset-b"],
				enabled: false,
			},
			{
				requestId: "bulk-delete",
				ids: ["asset-a", "asset-b"],
			},
		]);
	});

	it("retries and uploads a custom thumbnail for one stable media id", async () => {
		const fetchStub = vi.fn(
			async () =>
				new Response(
					JSON.stringify({ revision: 4, itemCount: 1, folders: [] }),
					{ status: 200, headers: { "content-type": "application/json" } },
				),
		);
		vi.stubGlobal("fetch", fetchStub);

		await api.retryLibraryThumbnail("asset/a", { requestId: "retry" });
		const image = new File(["pixels"], "custom.png", { type: "image/png" });
		await api.uploadLibraryThumbnail("asset/a", "custom", image);

		const calls = fetchStub.mock.calls as unknown as [string, RequestInit][];
		expect(calls[0][0]).toBe("/api/v2/library/items/asset%2Fa/thumbnail/retry");
		expect(JSON.parse(String(calls[0][1].body))).toEqual({
			requestId: "retry",
		});
		expect(calls[1][0]).toBe(
			"/api/v2/library/items/asset%2Fa/thumbnail/upload?requestId=custom",
		);
		expect(calls[1][1].body).toBeInstanceOf(FormData);
		expect((calls[1][1].body as FormData).get("file")).toBe(image);
	});

	it("triggers a payload-free live action with a plain GET", async () => {
		const fetchStub = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", fetchStub);

		await api.resetLayer("an-output", 0);

		const [url, init] = fetchStub.mock.calls[0] as unknown as [
			string,
			RequestInit | undefined,
		];
		expect(url).toBe("/api/v2/outputs/an-output/layers/0/reset");
		expect(init?.method).toBeUndefined();
	});
});
