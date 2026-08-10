import { describe, expect, it, vi } from "vitest";
import { TimecodesApiClient } from "./timecodes";
import type { ClientTransport } from "./transport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("TimecodesApiClient", () => {
	it("sends revision-safe portable mutations with a replay identity", async () => {
		const wire = transport();
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000070",
		);
		const client = new TimecodesApiClient(wire);
		await client.update("show-a", "timecode/a", 4, { name: "Opener" });
		const [path, init] = wire.request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/timecodes/actions");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		expect(JSON.parse(String(init.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000070",
			action: {
				type: "update",
				timecode_id: "timecode/a",
				expected_revision: 4,
				patch: { name: "Opener" },
			},
		});
	});

	it("uses the authoritative list, snapshot and transport routes", async () => {
		const wire = transport();
		const client = new TimecodesApiClient(wire);
		await client.objects("show-a");
		await client.runtime("show-a");
		await client.snapshot("show-a", "timecode/a");
		await client.transportAction("show-a", "timecode/a", {
			type: "seek",
			frame: 220,
		});
		expect(wire.request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/timecodes",
			"/api/v2/timecodes/runtime",
			"/api/v2/timecodes/timecode%2Fa/runtime",
			"/api/v2/timecodes/timecode%2Fa/transport",
		]);
	});

	it("discovers server outputs and uploads audio as a show-scoped raw import", async () => {
		const wire = transport();
		const client = new TimecodesApiClient(wire);
		await client.outputDevices();
		const file = new File(
			[new Uint8Array([0x49, 0x44, 0x33])],
			"intro mix.mp3",
			{
				type: "audio/mpeg",
			},
		);
		await client.importAudio("show-a", file);
		expect(wire.request.mock.calls[0]?.[0]).toBe(
			"/api/v2/timecodes/audio/outputs",
		);
		const [path, init] = wire.request.mock.calls[1] ?? [];
		expect(path).toBe("/api/v2/timecodes/audio/import?name=intro%20mix.mp3");
		expect(init.method).toBe("POST");
		expect(init.body).toBe(file);
		expect(new Headers(init.headers).get("content-type")).toBe("audio/mpeg");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
	});
});
