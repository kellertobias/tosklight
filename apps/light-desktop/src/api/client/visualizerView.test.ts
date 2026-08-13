import { describe, expect, it, vi } from "vitest";
import type { ClientTransport } from "./transport";
import { VisualizerViewApiClient } from "./visualizerView";

describe("VisualizerViewApiClient", () => {
	it("reads every renderer target the desk is driving", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			connected: true,
			views: [
				{
					target: "main",
					mode: "top_down",
					quality: "high",
					exposure: 1,
					ambient: 0.06,
					revision: 3,
					physics_reset_generation: 2,
				},
			],
		}));
		const client = new VisualizerViewApiClient({
			request,
		} as unknown as ClientTransport);

		await expect(client.snapshot()).resolves.toEqual({
			connected: true,
			views: [
				{
					target: "main",
					mode: "top_down",
					quality: "high",
					exposure: 1,
					ambient: 0.06,
					revision: 3,
					physicsResetGeneration: 2,
				},
			],
		});
		expect(request.mock.calls[0]?.[0]).toBe("/api/v2/visualizer-views");
	});

	it("posts an idempotent patch carrying only what changed", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			request_id: "request-id",
			view: {
				target: "main",
				mode: "lines_3d",
				quality: "high",
				exposure: 1,
				ambient: 0.06,
				revision: 4,
				physics_reset_generation: 0,
			},
			replayed: false,
			changed: true,
		}));
		const client = new VisualizerViewApiClient({
			request,
		} as unknown as ClientTransport);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000077",
		);

		const view = await client.update("main", { mode: "lines_3d" });

		expect(view.mode).toBe("lines_3d");
		const [path, init] = request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/visualizer-views/main/update");
		expect(init?.method).toBe("POST");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000077",
			patch: { mode: "lines_3d" },
		});
	});

	it("escapes a renderer target rather than pasting it into the path", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			request_id: "request-id",
			view: {
				target: "front-of-house",
				mode: "simple_3d",
				quality: "draft",
				exposure: 1,
				ambient: 0.06,
				revision: 1,
				physics_reset_generation: 0,
			},
			replayed: false,
			changed: true,
		}));
		const client = new VisualizerViewApiClient({
			request,
		} as unknown as ClientTransport);

		await client.update("front of house", { quality: "draft" });

		expect(request.mock.calls[0]?.[0]).toBe(
			"/api/v2/visualizer-views/front%20of%20house/update",
		);
	});
});
