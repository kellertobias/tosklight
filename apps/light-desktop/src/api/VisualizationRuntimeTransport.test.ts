import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { frontendPerformanceDiagnostics } from "../features/frontendWarmup/diagnostics";
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
const wireScope = { show_id: SHOW_ID };

class FakeWebSocket extends EventTarget {
	static readonly OPEN = 1;
	static instances: FakeWebSocket[] = [];
	readonly sent: string[] = [];
	readyState = 0;

	constructor(
		readonly url: string | URL,
		readonly protocols?: string | string[],
	) {
		super();
		FakeWebSocket.instances.push(this);
	}

	open() {
		this.readyState = FakeWebSocket.OPEN;
		this.dispatchEvent(new Event("open"));
	}

	message(value: unknown) {
		this.dispatchEvent(
			new MessageEvent("message", { data: JSON.stringify(value) }),
		);
	}

	send(value: string) {
		this.sent.push(value);
	}

	close() {
		this.readyState = 3;
		this.dispatchEvent(new Event("close"));
	}
}

describe("HttpVisualizationRuntimeTransport", () => {
	beforeEach(() => {
		vi.spyOn(Date, "now").mockReturnValue(
			Date.parse("2026-07-21T09:00:00.050Z"),
		);
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("sends the final unsubscribe before closing the shared socket", () => {
		FakeWebSocket.instances = [];
		const stream = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			FakeWebSocket as unknown as typeof WebSocket,
		).openStream(scope, { snapshot: vi.fn(), error: vi.fn() });
		stream.updateClaims(["normal"], 10);
		const socket = FakeWebSocket.instances[0];
		socket?.open();

		stream.updateClaims([], 10);

		expect(socket?.sent.map((message) => JSON.parse(message))).toContainEqual({
			type: "unsubscribe",
			lanes: ["normal"],
		});
		expect(socket?.readyState).toBe(3);
	});

	it("loads only the exact v1 Visualization endpoint with authenticated headers", async () => {
		const diagnosticCount =
			frontendPerformanceDiagnostics.snapshot().stage.visualizationRequests
				.length;
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
			"http://desk.test/api/v2/output/visualization",
		);
		const headers = fetch.mock.calls[0]?.[1]?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(headers.get("x-tosk-show")).toBe(SHOW_ID);
		expect(fetch.mock.calls[0]?.[0]).not.toContain("bootstrap");
		expect(fetch.mock.calls[0]?.[0]).not.toContain("playbacks");
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.visualizationRequests[
				diagnosticCount
			],
		).toMatchObject({
			lane: "normal",
			status: "ready",
			payloadBytes: new TextEncoder().encode(JSON.stringify(snapshot(false)))
				.byteLength,
			receivedAt: expect.any(Number),
			durationMs: expect.any(Number),
		});
	});

	it("keeps the preload lane on its independent query", async () => {
		const fetch = vi.fn(
			async (
				_input: Parameters<typeof globalThis.fetch>[0],
				_init?: Parameters<typeof globalThis.fetch>[1],
			) => response(snapshot(true)),
		);
		const transport = createTransport(fetch);

		await expect(
			transport.loadSnapshot(scope, "preload"),
		).resolves.toMatchObject({
			preload: true,
		});
		expect(fetch.mock.calls[0]?.[0]).toBe(
			"http://desk.test/api/v2/output/visualization?preload=true",
		);
	});

	it("requests a lightweight Dynamic-stack-only projection for paperwork", async () => {
		const fetch = vi.fn(
			async (
				_input: Parameters<typeof globalThis.fetch>[0],
				_init?: Parameters<typeof globalThis.fetch>[1],
			) => response(snapshot(false)),
		);
		const transport = createTransport(fetch);

		await transport.loadSnapshot(scope, "normal", {
			dynamicStackOnly: true,
			fixtureIds: ["fixture-1", "fixture-2"],
		});

		expect(fetch.mock.calls[0]?.[0]).toBe(
			"http://desk.test/api/v2/output/visualization?dynamic_stack_only=true&fixture_ids=fixture-1%2Cfixture-2",
		);
	});

	it("correlates HTTP source age without treating a one-shot read as a Stage frame", async () => {
		const baseline = frontendPerformanceDiagnostics.snapshot().stage;
		const frameCount = baseline.frames.length;
		const requestCount = baseline.visualizationRequests.length;
		const sourceTimestamp = new Date(Date.now() - 25).toISOString();
		const fetch = vi.fn(async () =>
			response({
				...snapshot(false),
				generated_at: new Date().toISOString(),
				source_frame: 42,
				source_timestamp: sourceTimestamp,
			}),
		);

		const decoded = await createTransport(fetch).loadSnapshot(scope, "normal");

		expect(decoded).not.toHaveProperty("source_frame");
		expect(decoded).not.toHaveProperty("source_timestamp");
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.visualizationRequests[
				requestCount
			],
		).toMatchObject({
			sourceGeneratedAt: sourceTimestamp,
			sourceAgeMs: expect.any(Number),
		});
		expect(frontendPerformanceDiagnostics.snapshot().stage.frames).toHaveLength(
			frameCount,
		);
	});

	it("multiplexes claimed lanes over the dedicated authenticated stream", async () => {
		FakeWebSocket.instances = [];
		const transport = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			FakeWebSocket as unknown as typeof WebSocket,
		);
		const observer = { snapshot: vi.fn(), error: vi.fn() };
		const stream = transport.openStream(scope, observer);
		stream.updateClaims(["normal", "preload"], 10);

		const socket = FakeWebSocket.instances[0];
		expect(socket?.url.toString()).toBe(
			"ws://desk.test/api/v2/visualization/stream",
		);
		expect(socket?.protocols).toEqual([
			"light.visualization.v1",
			"light.token.session-token",
		]);
		socket?.open();
		expect(JSON.parse(socket?.sent[0] ?? "")).toEqual({
			type: "subscribe",
			lanes: ["normal", "preload"],
			max_rate_hz: 10,
			include_dynamic_stack: false,
			sparse_dynamic_stack: true,
			batched_messages: true,
			acknowledgements: false,
		});
		socket?.message({
			type: "hello",
			protocol_version: 1,
			max_rate_hz: 10,
			lanes: ["normal", "preload"],
			scope: wireScope,
		});
		socket?.message({
			type: "snapshot",
			lane: "normal",
			scope: wireScope,
			sequence: 1,
			source_frame: 7,
			source_timestamp: "2026-07-21T09:00:00Z",
			published_at: "2026-07-21T09:00:00Z",
			snapshot: {
				...snapshot(false),
				dynamic_stack: [dynamicStackEntry()],
			},
		});
		await nextTask();

		expect(observer.snapshot).toHaveBeenCalledWith(
			"normal",
			expect.objectContaining({ revision: 7, preload: false }),
		);
		expect(
			socket?.sent.map((message) => JSON.parse(message)),
		).not.toContainEqual({
			type: "acknowledge",
			sequence: 1,
		});
		expect(observer.error).not.toHaveBeenCalled();
		socket?.message({
			type: "delta",
			lane: "normal",
			scope: wireScope,
			sequence: 2,
			source_frame: 8,
			source_timestamp: "2026-07-21T09:00:00.100Z",
			published_at: "2026-07-21T09:00:00.101Z",
			delta: {
				scope: wireScope,
				revision: 7,
				generated_at: "2026-07-21T09:00:00.100Z",
				grand_master: 0.8,
				blackout: false,
				preload: false,
				values: [
					{
						fixture_id: "fixture-1",
						attribute: "intensity",
						value: { kind: "normalized", value: 0.75 },
					},
				],
				removed_values: [],
				profile_output_values: [],
				removed_profile_output_values: [],
			},
		});
		await nextTask();
		expect(observer.snapshot).toHaveBeenLastCalledWith(
			"normal",
			expect.objectContaining({
				dynamic_stack: [expect.objectContaining({ name: "Pulse" })],
				values: [
					expect.objectContaining({
						value: { kind: "normalized", value: 0.75 },
					}),
				],
			}),
		);
		expect(
			socket?.sent.map((message) => JSON.parse(message)),
		).not.toContainEqual({
			type: "acknowledge",
			sequence: 2,
		});
		socket?.message({
			type: "delta",
			lane: "normal",
			scope: wireScope,
			sequence: 3,
			source_frame: 9,
			source_timestamp: "2026-07-21T09:00:00.200Z",
			published_at: "2026-07-21T09:00:00.201Z",
			delta: {
				scope: wireScope,
				revision: 7,
				generated_at: "2026-07-21T09:00:00.200Z",
				grand_master: 0.8,
				blackout: false,
				preload: false,
				values: [],
				removed_values: [],
				dynamic_stack: [],
				profile_output_values: [],
				removed_profile_output_values: [],
			},
		});
		await nextTask();
		expect(observer.snapshot).toHaveBeenLastCalledWith(
			"normal",
			expect.objectContaining({ dynamic_stack: [] }),
		);
		socket?.close();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Visualization stream closed; reconnecting",
			}),
		);
		stream.close();
	});

	it("installs a settled full snapshot when reconnecting", () => {
		FakeWebSocket.instances = [];
		const observer = { snapshot: vi.fn(), error: vi.fn() };
		const stream = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			FakeWebSocket as unknown as typeof WebSocket,
		).openStream(scope, observer);
		stream.updateClaims(["normal"], 10);
		const socket = FakeWebSocket.instances[0];
		socket?.open();

		socket?.message({
			type: "snapshot",
			lane: "normal",
			scope: wireScope,
			sequence: 1,
			source_frame: 7,
			source_timestamp: "2026-07-21T08:59:59Z",
			published_at: "2026-07-21T09:00:00.050Z",
			snapshot: snapshot(false),
		});

		expect(observer.snapshot).toHaveBeenCalledWith(
			"normal",
			expect.objectContaining({ revision: 7 }),
		);
		expect(observer.error).not.toHaveBeenCalled();
		stream.close();
	});

	it("merges delta upserts and removals without losing canonical value order", () => {
		FakeWebSocket.instances = [];
		const observer = { snapshot: vi.fn(), error: vi.fn() };
		const stream = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			FakeWebSocket as unknown as typeof WebSocket,
		).openStream(scope, observer);
		stream.updateClaims(["normal"], 10);
		const socket = FakeWebSocket.instances[0];
		socket?.open();
		const value = (fixture_id: string, attribute: string, level: number) => ({
			fixture_id,
			attribute,
			value: { kind: "normalized", value: level },
		});
		socket?.message({
			type: "snapshot",
			lane: "normal",
			scope: wireScope,
			sequence: 1,
			source_frame: 1,
			source_timestamp: "2026-07-21T09:00:00Z",
			published_at: "2026-07-21T09:00:00Z",
			snapshot: {
				...snapshot(false),
				values: [
					value("fixture-2", "intensity", 0.2),
					value("fixture-1", "pan", 0.1),
					value("fixture-1", "intensity", 0.1),
				],
			},
		});
		socket?.message({
			type: "delta",
			lane: "normal",
			scope: wireScope,
			sequence: 2,
			source_frame: 2,
			source_timestamp: "2026-07-21T09:00:00.100Z",
			published_at: "2026-07-21T09:00:00.101Z",
			delta: {
				scope: wireScope,
				revision: 8,
				generated_at: "2026-07-21T09:00:00.100Z",
				grand_master: 0.8,
				blackout: false,
				preload: false,
				values: [
					value("fixture-3", "intensity", 0.3),
					value("fixture-1", "intensity", 0.9),
				],
				removed_values: [{ fixture_id: "fixture-1", attribute: "pan" }],
				dynamic_stack: [],
				profile_output_values: [],
				removed_profile_output_values: [],
			},
		});

		expect(
			observer.snapshot.mock.calls
				.at(-1)?.[1]
				.values.map(
					(entry: { fixture_id: string; attribute: string }) =>
						`${entry.fixture_id}:${entry.attribute}`,
				),
		).toEqual([
			"fixture-1:intensity",
			"fixture-2:intensity",
			"fixture-3:intensity",
		]);
		expect(observer.error).not.toHaveBeenCalled();
		stream.close();
	});

	it("rejects stream messages scoped to a replacement Show", () => {
		FakeWebSocket.instances = [];
		const observer = { snapshot: vi.fn(), error: vi.fn() };
		const stream = createTransport(
			vi.fn<typeof globalThis.fetch>(),
			FakeWebSocket as unknown as typeof WebSocket,
		).openStream(scope, observer);
		stream.updateClaims(["normal"], 10);
		const socket = FakeWebSocket.instances[0];
		socket?.open();

		socket?.message({
			type: "snapshot",
			lane: "normal",
			scope: { show_id: "33333333-3333-4333-8333-333333333333" },
			sequence: 1,
			source_frame: 7,
			source_timestamp: "2026-07-21T09:00:00Z",
			published_at: "2026-07-21T09:00:00Z",
			snapshot: {
				...snapshot(false),
				scope: { show_id: "33333333-3333-4333-8333-333333333333" },
			},
		});

		expect(observer.snapshot).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				message: expect.stringContaining("configured visualization Show scope"),
			}),
		);
		stream.close();
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
			transport.loadSnapshot({ ...scope, authorityKey: "server-b" }, "normal"),
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
		const decoded = decodeVisualizationRuntimeSnapshot(
			snapshot(false),
			"normal",
		);

		expect(decoded.values).toEqual([
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
			},
		]);
		expect(decoded.profile_output_values).toEqual(decoded.values);
	});

	it("decodes the Dynamic stack emitted by the production endpoint", () => {
		const decoded = decodeVisualizationRuntimeSnapshot(
			{
				...snapshot(false),
				dynamic_stack: [dynamicStackEntry()],
			},
			"normal",
		);

		expect(decoded.dynamic_stack).toEqual([
			expect.objectContaining({
				entry_type: "dynamic",
				priority: -10,
				activation_mix: 0.5,
				winning: true,
			}),
		]);
	});

	it("tolerates unknown fields for forward-compatible Visualization payloads", () => {
		expect(
			decodeVisualizationRuntimeSnapshot(
				{
					...snapshot(false),
					future_projection: { version: 3 },
					values: [
						{
							...(snapshot(false).values as Array<Record<string, unknown>>)[0],
							future_value_metadata: { source: "new-server" },
						},
					],
					profile_output_values: [
						{
							...(
								snapshot(false).profile_output_values as Array<
									Record<string, unknown>
								>
							)[0],
							future_profile_metadata: true,
						},
					],
				},
				"normal",
			),
		).toMatchObject({ revision: 7, preload: false });
	});

	it.each([
		["missing lane", without(snapshot(false), "preload")],
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

function nextTask() {
	return new Promise<void>((resolve) => globalThis.setTimeout(resolve, 10));
}

function createTransport(
	fetch: typeof globalThis.fetch,
	webSocket?: typeof globalThis.WebSocket,
) {
	return new HttpVisualizationRuntimeTransport({
		baseUrl: "http://desk.test/",
		sessionToken: "session-token",
		showId: SHOW_ID,
		sessionId: SESSION_ID,
		authorityKey: "server-a",
		deskBoundaryToken: "desk-token",
		fetch,
		webSocket,
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
		scope: wireScope,
		revision: 7,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 0.8,
		blackout: false,
		preload,
		values,
		profile_output_values: values,
	};
}

function dynamicStackEntry() {
	return {
		fixture_id: "fixture-1",
		attribute: "intensity",
		entry_type: "dynamic",
		priority: -10,
		changed_at_millis: 1_234,
		source: "Programmer",
		dynamic_id: "33333333-3333-4333-8333-333333333333",
		pool_number: 1,
		name: "Pulse",
		runtime_instance_id: "44444444-4444-4444-8444-444444444444",
		controller_id: "55555555-5555-4555-8555-555555555555",
		lane_id: "66666666-6666-4666-8666-666666666666",
		size: 0.75,
		activation_mix: 0.5,
		paused: false,
		hidden: false,
		pending: false,
		winning: true,
		value: { kind: "normalized", value: 0.4 },
		resolved_value: { kind: "normalized", value: 0.4 },
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
