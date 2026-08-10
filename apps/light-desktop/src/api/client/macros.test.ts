import { describe, expect, it, vi } from "vitest";
import { MacrosApiClient } from "./macros";
import type { ClientTransport } from "./transport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("MacrosApiClient", () => {
	it("creates portable Macros through an idempotent show-scoped action", async () => {
		const wire = transport();
		const client = new MacrosApiClient(wire);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000071",
		);

		await client.create("show-a", {
			id: "10000000-0000-4000-8000-000000000071",
			number: 71,
			name: "Preset fixtures",
			source: "Fixture 1 At 50\nStore Preset 1",
			presentation: { color: "#8f3541" },
		});

		const [path, init] = wire.request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/macros/actions");
		expect(init.method).toBe("POST");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		expect(JSON.parse(String(init.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000071",
			action: {
				type: "create",
				definition: {
					id: "10000000-0000-4000-8000-000000000071",
					number: 71,
					name: "Preset fixtures",
					source: "Fixture 1 At 50\nStore Preset 1",
					presentation: { color: "#8f3541" },
				},
			},
		});
	});

	it("uses typed run, run-line, guarded undo, runtime and cancellation routes", async () => {
		const wire = transport();
		const client = new MacrosApiClient(wire);

		await client.run("show-a", "macro/a", {
			source_revision: 4,
			trigger: { type: "pool" },
		});
		await client.runLine("show-a", "macro/a", {
			source_revision: 4,
			line: 2,
		});
		await client.undoRunLine("show-a", "execution/a");
		await client.runtime("show-a");
		await client.cancel("show-a", "execution/a");

		expect(wire.request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/macros/macro%2Fa/run",
			"/api/v2/macros/macro%2Fa/run-line",
			"/api/v2/macros/executions/execution%2Fa/undo-line",
			"/api/v2/macros/runtime",
			"/api/v2/macros/executions/cancel",
		]);
		for (const [, init] of wire.request.mock.calls) {
			expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		}
		expect(JSON.parse(String(wire.request.mock.calls[4]?.[1].body))).toEqual({
			execution_id: "execution/a",
		});
	});
});
