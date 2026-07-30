import { describe, expect, it, vi } from "vitest";
import { SchedulesApiClient } from "./schedules";
import type { ClientTransport } from "./transport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("SchedulesApiClient", () => {
	it("scopes snapshots and previews to the active show", async () => {
		const wire = transport();
		const client = new SchedulesApiClient(wire);

		await client.snapshot("show-a");
		expect(wire.request).toHaveBeenLastCalledWith("/api/v2/schedules", {
			headers: { "x-tosk-show": "show-a" },
		});

		const abort = new AbortController();
		await client.preview(
			"show-a",
			{
				trigger: {
					type: "calendar",
					rule: { type: "expression", expression: "0 14 * * 1" },
				},
				count: 5,
			},
			abort.signal,
		);
		const [path, init] = wire.request.mock.calls.at(-1) ?? [];
		expect(path).toBe("/api/v2/schedules/preview");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("show-a");
		expect(init.signal).toBe(abort.signal);
	});

	it("creates idempotent typed mutations with page slot and stable Playback identity", async () => {
		const wire = transport();
		const client = new SchedulesApiClient(wire);
		await client.create("show-a", {
			name: "Doors",
			enabled: true,
			trigger: {
				type: "interval",
				every_seconds: 300,
				enabled_at: "2030-01-01T00:00:00Z",
			},
			target: {
				type: "playback",
				page: 2,
				slot: 4,
				playback_number: 17,
				action: "go",
				master_transition: null,
			},
		});

		const [path, init] = wire.request.mock.calls.at(-1) ?? [];
		expect(path).toBe("/api/v2/schedules/create");
		const body = JSON.parse(String(init.body));
		expect(body.request_id).toMatch(
			/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu,
		);
		expect(body.definition.target).toEqual({
			type: "playback",
			page: 2,
			slot: 4,
			playback_number: 17,
			action: "go",
			master_transition: null,
		});
	});
});
