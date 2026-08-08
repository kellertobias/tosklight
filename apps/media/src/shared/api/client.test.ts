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
		answering({ code: "dmx-owns-this", message: "a desk is driving this output" }, 409);

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

		const failure = (await api.health().catch((error: unknown) => error)) as ApiFailure;
		expect(failure.disconnected).toBe(true);
		expect(failure.code).toBe("unreachable");
	});

	it("still produces a readable failure when something answers that is not the API", async () => {
		vi.stubGlobal("fetch", vi.fn(async () => new Response("<html>proxy error</html>", { status: 502 })));

		const failure = (await api.catalog().catch((error: unknown) => error)) as ApiFailure;
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

		const [url, init] = fetchStub.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe("/api/v2/outputs/an-output/layers/2/update");
		expect(init.body).toBe('{"dimmer":0.5}');
	});

	it("triggers a payload-free live action with a plain GET", async () => {
		const fetchStub = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", fetchStub);

		await api.resetLayer("an-output", 0);

		const [url, init] = fetchStub.mock.calls[0] as unknown as [string, RequestInit | undefined];
		expect(url).toBe("/api/v2/outputs/an-output/layers/0/reset");
		expect(init?.method).toBeUndefined();
	});
});
