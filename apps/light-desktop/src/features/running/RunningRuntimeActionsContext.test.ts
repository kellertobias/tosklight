import { describe, expect, it, vi } from "vitest";
import { TimecodeRunningApiClient } from "../../api/client/runningRuntime";
import type { ClientTransport } from "../../api/client/transport";

function transport() {
	const request = vi.fn().mockResolvedValue({});
	request.mockImplementation((path: string) =>
		Promise.resolve(
			path.endsWith("/runtime")
				? []
				: {
						timecode_id: "timecode/a",
						revision: 1,
						state: "stopped",
						frame: 0,
						duration_frame: 1,
						audio_linked: false,
					},
		),
	);
	return {
		request,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("TimecodeRunningApiClient", () => {
	it("uses the show-scoped runtime and authoritative stop routes", async () => {
		const wire = transport();
		const client = new TimecodeRunningApiClient(wire);

		await client.runtime("show-a");
		await client.stop("show-a", "timecode/a");

		expect(wire.request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/timecodes/runtime",
			"/api/v2/timecodes/timecode%2Fa/transport",
		]);
		for (const [, init] of wire.request.mock.calls) {
			expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		}
		expect(JSON.parse(String(wire.request.mock.calls[1]?.[1].body))).toEqual({
			timecode_id: "timecode/a",
			action: { type: "stop" },
		});
	});
});
