// A stand-in for the Media Server.
//
// Tests drive the real client and the real resource cache; only the socket is replaced. That way
// a test proving a rollback is proving the code an operator runs, not a mock of it.

import { vi } from "vitest";
import type { CatalogView, Health, OutputView } from "../shared/api/generated/media-wire";
import { resetResources } from "../shared/api/resource";

export interface StubbedServer {
	outputs: OutputView[];
	catalog: CatalogView;
	health: Health;
	/** Set to make the next write fail with this code and status. */
	refuseWrites: { code: string; message: string; status: number } | undefined;
	/** Every path written to, in order. */
	writes: string[];
}

export function stubServer(overrides: Partial<StubbedServer> = {}): StubbedServer {
	const server: StubbedServer = {
		outputs: [anOutput()],
		catalog: aCatalog(),
		health: {
			status: "ok",
			instance: "test-instance",
			outputs: 1,
			catalogRevision: 3,
			catalogItems: 2,
		},
		refuseWrites: undefined,
		writes: [],
		...overrides,
	};

	resetResources();
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input).replace("/api/v2", "");
			if (init?.method === "POST" || path.endsWith("/reset")) {
				server.writes.push(path);
				if (server.refuseWrites) {
					const { code, message, status } = server.refuseWrites;
					return jsonResponse({ code, message }, status);
				}
			}
			if (path === "/health") return jsonResponse(server.health);
			if (path === "/catalog") return jsonResponse(server.catalog);
			if (path === "/outputs") return jsonResponse(server.outputs);
			if (path.endsWith("/reset")) return new Response(null, { status: 204 });

			const update = path.match(/^\/outputs\/([^/]+)\/layers\/(\d+)\/update$/u);
			if (update) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				const output = server.outputs.find((candidate) => candidate.id === update[1]);
				if (!output) return jsonResponse({ code: "unknown-output", message: "no" }, 404);
				const layer = output.layers[Number(update[2])];
				if (body.dimmer !== undefined) layer.dimmer = body.dimmer;
				if (body.folder !== undefined) layer.address.folder = body.folder;
				if (body.file !== undefined) layer.address.file = body.file;
				return jsonResponse(output);
			}
			return jsonResponse({ code: "unknown-route", message: path }, 404);
		}),
	);
	return server;
}

function jsonResponse(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { "content-type": "application/json" },
	});
}

export function anOutput(overrides: Partial<OutputView> = {}): OutputView {
	return {
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
		layerCount: 2,
		dmxActive: false,
		master: {
			dimmer: 1,
			volume: 1,
			tintRed: 1,
			tintGreen: 1,
			tintBlue: 1,
			flipMirror: "none",
			mask: { folder: 0, file: 0, class: "blank" },
		},
		layers: [aLayer(0), aLayer(1)],
		...overrides,
	};
}

export function aLayer(index: number): OutputView["layers"][number] {
	return {
		index,
		address: { folder: 1, file: index + 1, class: "library" },
		playMode: "Loop",
		dimmer: 1,
		scaleX: 1,
		scaleY: 1,
		positionX: 0,
		positionY: 0,
		rotation: 0,
		grayscale: 0,
		sourceStatus: { state: "ready", failure: null },
		drawing: true,
	};
}

export function aCatalog(): CatalogView {
	return {
		revision: 3,
		itemCount: 2,
		folders: [
			{
				folder: 1,
				name: "Looks",
				items: [
					{
						id: "asset-a",
						file: 1,
						name: "Blue haze",
						kind: "video",
						width: 1920,
						height: 1080,
						frames: 600,
						intrinsicBpm: 120,
					},
					{
						id: "asset-b",
						file: 2,
						name: "Static grid",
						kind: "image",
						width: 1920,
						height: 1080,
						frames: null,
						intrinsicBpm: null,
					},
				],
			},
		],
	};
}
