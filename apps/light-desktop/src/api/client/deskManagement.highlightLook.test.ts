import { describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../types";
import { DeskManagementApiClient } from "./deskManagement";
import type { LiveClientTransport } from "./transport";

describe("Desk Management Highlight Look configuration", () => {
	it("serializes the semantic look and preserves every Ignore value as null", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			configuration: {},
			requires_restart: false,
			matter: {},
			request_id: "configuration-1",
			replayed: false,
		}));
		const client = new DeskManagementApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			currentDeskId: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);
		const configuration = {
			frame_rate_hz: 44,
			highlight_look: {
				intensity: 0.75,
				color: "cyan",
				iris: null,
				zoom: 0.5,
				focus: null,
				frost: null,
				compatibility: "needs_review",
			},
		} as DeskConfiguration;

		await client.updateConfiguration(configuration);

		const [, init] = request.mock.calls[0];
		const body = JSON.parse(String(init?.body));
		expect(body.patch.highlight_look).toEqual(configuration.highlight_look);
	});
});
