import { describe, expect, it, vi } from "vitest";
import type { VisualizationRuntimeScope } from "../features/visualizationRuntime/contracts";
import { VisualizationRuntimeProtocolError } from "../features/visualizationRuntime/transport";
import {
	decodeVisualizationRuntimeSnapshot,
	HttpVisualizationRuntimeTransport,
} from "./VisualizationRuntimeTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";
const scope: VisualizationRuntimeScope = {
	showId: SHOW_ID,
	sessionId: SESSION_ID,
	authorityKey: "server-a",
};

describe("HttpVisualizationRuntimeTransport", () => {
	it("loads only the exact v1 Visualization endpoint with authenticated headers", async () => {
		const fetch = vi.fn(
			async (
				_input: Parameters<typeof globalThis.fetch>[0],
				_init?: Parameters<typeof globalThis.fetch>[1],
			) => response(snapshot(false)),
		);
		const transport = createTransport(fetch);

		const decoded = await transport.loadSnapshot(scope, "normal");

		expect(decoded).toMatchObject({ revision: 7, preload: false });
		expect(fetch).toHaveBeenCalledOnce();
		expect(fetch.mock.calls[0]?.[0]).toBe(
			"http://desk.test/api/v1/visualization",
		);
		const headers = fetch.mock.calls[0]?.[1]?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(fetch.mock.calls[0]?.[0]).not.toContain("bootstrap");
		expect(fetch.mock.calls[0]?.[0]).not.toContain("playbacks");
	});

	it("keeps the preload lane on its independent query", async () => {
		const fetch = vi.fn(
			async (
				_input: Parameters<typeof globalThis.fetch>[0],
				_init?: Parameters<typeof globalThis.fetch>[1],
			) => response(snapshot(true)),
		);
		const transport = createTransport(fetch);

		await expect(transport.loadSnapshot(scope, "preload")).resolves.toMatchObject({
			preload: true,
		});
		expect(fetch.mock.calls[0]?.[0]).toBe(
			"http://desk.test/api/v1/visualization?preload=true",
		);
	});

	it("rejects a foreign Show, session, or server before issuing a request", async () => {
		const fetch = vi.fn(
			async (
				_input: Parameters<typeof globalThis.fetch>[0],
				_init?: Parameters<typeof globalThis.fetch>[1],
			) => response(snapshot(false)),
		);
		const transport = createTransport(fetch);

		await expect(
			transport.loadSnapshot(
				{ ...scope, showId: "33333333-3333-4333-8333-333333333333" },
				"normal",
			),
		).rejects.toBeInstanceOf(VisualizationRuntimeProtocolError);
		await expect(
			transport.loadSnapshot(
				{ ...scope, sessionId: "44444444-4444-4444-8444-444444444444" },
				"normal",
			),
		).rejects.toBeInstanceOf(VisualizationRuntimeProtocolError);
		await expect(
			transport.loadSnapshot(
				{ ...scope, authorityKey: "server-b" },
				"normal",
			),
		).rejects.toBeInstanceOf(VisualizationRuntimeProtocolError);
		expect(fetch).not.toHaveBeenCalled();
	});

	it("strictly rejects a response from the other lane", async () => {
		const transport = createTransport(
			vi.fn(
				async (
					_input: Parameters<typeof globalThis.fetch>[0],
					_init?: Parameters<typeof globalThis.fetch>[1],
				) => response(snapshot(true)),
			),
		);

		await expect(transport.loadSnapshot(scope, "normal")).rejects.toThrow(
			"response belongs to the preload lane",
		);
	});
});

describe("decodeVisualizationRuntimeSnapshot", () => {
	it("decodes resolved and post-profile attribute values", () => {
		const decoded = decodeVisualizationRuntimeSnapshot(snapshot(false), "normal");

		expect(decoded.values).toEqual([
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
			},
		]);
		expect(decoded.profile_output_values).toEqual(decoded.values);
	});

	it.each([
		["missing lane", without(snapshot(false), "preload")],
		["unknown field", { ...snapshot(false), foreign_scope: "other" }],
		["invalid master", { ...snapshot(false), grand_master: 1.1 }],
		["invalid timestamp", { ...snapshot(false), generated_at: "later" }],
		[
			"malformed value",
			{ ...snapshot(false), values: [{ fixture_id: "fixture-1" }] },
		],
	])("rejects %s", (_label, value) => {
		expect(() => decodeVisualizationRuntimeSnapshot(value, "normal")).toThrow();
	});
});

function createTransport(fetch: typeof globalThis.fetch) {
	return new HttpVisualizationRuntimeTransport({
		baseUrl: "http://desk.test/",
		sessionToken: "session-token",
		showId: SHOW_ID,
		sessionId: SESSION_ID,
		authorityKey: "server-a",
		deskBoundaryToken: "desk-token",
		fetch,
	});
}

function snapshot(preload: boolean) {
	const values = [
		{
			fixture_id: "fixture-1",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.5 },
		},
	];
	return {
		revision: 7,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 0.8,
		blackout: false,
		preload,
		values,
		profile_output_values: values,
	};
}

function response(value: unknown) {
	return new Response(JSON.stringify(value), {
		status: 200,
		headers: { "content-type": "application/json" },
	});
}

function without(value: Record<string, unknown>, key: string) {
	const copy = { ...value };
	delete copy[key];
	return copy;
}
