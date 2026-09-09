import { describe, expect, it, vi } from "vitest";
import type { ControlDesk } from "../types";
import { PlaybackApiClient } from "./playback";
import type { LiveClientTransport } from "./transport";

describe("hardware lighting intent", () => {
	it("sends only the changed lighting values and retains request identity", async () => {
		const desk: ControlDesk = { id: "desk-1", name: "Main", columns: 10, rows: 1, buttons: 3, hardware_led_brightness: 80, hardware_gooseneck_brightness: 60, hardware_gooseneck_color: 100 };
		const next = { ...desk, hardware_led_brightness: 0, hardware_gooseneck_color: 40 };
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({ desk: next }));
		const client = new PlaybackApiClient({ request } as unknown as LiveClientTransport);
		expect(await client.updateControlDesk(next, { ...desk, name: "A remotely changed name" }, { hardware_led_brightness: 0, hardware_gooseneck_color: 40 })).toEqual(next);
		const [path, init] = request.mock.calls[0];
		expect(path).toBe("/api/v2/control-desk/actions");
		expect(init?.method).toBe("POST");
		const body = JSON.parse(String(init?.body));
		expect(body.request_id).toMatch(/^[0-9a-f-]{36}$/);
		expect(body.action).toEqual({ type: "update", patch: { hardware_led_brightness: 0, hardware_gooseneck_color: 40 } });
	});
});
