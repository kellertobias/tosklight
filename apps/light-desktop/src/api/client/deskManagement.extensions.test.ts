import { describe, expect, it, vi } from "vitest";
import { DeskManagementApiClient } from "./deskManagement";
import type { LiveClientTransport } from "./transport";

describe("Desk Management native extensions", () => {
	it("loads status and rescans with a replay-safe request ID", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			packages: [],
			instances: [],
		}));
		const client = new DeskManagementApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			currentDeskId: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);

		await client.extensions();
		await client.rescanExtensions();

		expect(request.mock.calls[0]).toEqual(["/api/v2/extensions"]);
		expect(request.mock.calls[1]?.[0]).toBe("/api/v2/extensions/rescan");
		const init = request.mock.calls[1]?.[1] as RequestInit;
		expect(init.method).toBe("POST");
		expect(JSON.parse(String(init.body))).toEqual({
			request_id: expect.any(String),
		});
	});

	it("updates USB DMX endpoints with revision and request identity", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			document: { revision: 5, endpoints: [] },
		}));
		const client = new DeskManagementApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			currentDeskId: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);

		await client.updateUsbDmxEndpoints(4, {
			action: "remove",
			endpoint_id: "front-usb",
		});

		expect(request.mock.calls[0]?.[0]).toBe("/api/v2/usb-dmx/endpoints/update");
		const init = request.mock.calls[0]?.[1] as RequestInit;
		expect(JSON.parse(String(init.body))).toEqual({
			request_id: expect.any(String),
			expected_revision: 4,
			action: { action: "remove", endpoint_id: "front-usb" },
		});
	});
});
