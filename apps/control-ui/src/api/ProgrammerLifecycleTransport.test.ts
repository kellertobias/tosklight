import { describe, expect, it, vi } from "vitest";
import type { ProgrammerLifecycleProtocolError } from "../features/programmerLifecycle/transport";
import {
	HttpProgrammerLifecycleTransport,
	type ProgrammerLifecycleHttpError,
} from "./ProgrammerLifecycleTransport";

const PROGRAMMER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const SESSION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

class FakeWebSocket {
	static readonly OPEN = 1;
	static instances: FakeWebSocket[] = [];
	readonly readyState = FakeWebSocket.OPEN;
	readonly sent: string[] = [];
	readonly close = vi.fn();
	private readonly listeners = new Map<string, Array<(event: Event) => void>>();

	constructor(
		readonly url: string | URL,
		readonly protocols: string[],
	) {
		FakeWebSocket.instances.push(this);
	}

	addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
		const callback =
			typeof listener === "function"
				? listener
				: (event: Event) => listener.handleEvent(event);
		this.listeners.set(type, [...(this.listeners.get(type) ?? []), callback]);
	}

	send(value: string) {
		this.sent.push(value);
	}

	emit(type: string, event: Event = new Event(type)) {
		for (const listener of this.listeners.get(type) ?? []) listener(event);
	}
}

function row() {
	return {
		programmer_id: PROGRAMMER_ID,
		user_id: USER_ID,
		connected: true,
		selected_fixture_count: 2,
		normal_value_count: 1,
		sessions: [{ session_id: SESSION_ID }],
	};
}

function eventMessage() {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: { capability: "programmer", id: "programming-lifecycle" },
			related_objects: [],
			source: { kind: "runtime" },
			correlation_id: null,
			delivery: "lossless",
			payload: {
				type: "programming_lifecycle_changed",
				change: {
					revision: 2,
					delta: { type: "upsert", programmer: row() },
				},
			},
		},
	};
}

function message(value: unknown) {
	return { data: JSON.stringify(value) } as MessageEvent;
}

function harness(fetch = vi.fn<typeof globalThis.fetch>()) {
	FakeWebSocket.instances = [];
	const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
	const transport = new HttpProgrammerLifecycleTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpProgrammerLifecycleTransport", () => {
	it("stays dormant until a snapshot or subscription is requested", () => {
		const { fetch } = harness();
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("loads and strictly decodes the authenticated aggregate snapshot", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 11 },
					projection: { revision: 1, programmers: [row()] },
				}),
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.loadSnapshot()).resolves.toMatchObject({
			cursor: 11,
			projection: { revision: 1 },
		});
		const [url, options] = fetch.mock.calls[0];
		expect(url).toBe(
			"http://127.0.0.1:5000/api/v2/programmer-lifecycle/snapshot",
		);
		const headers = options?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("maps HTTP failures without accepting them as snapshots", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(JSON.stringify({ error: "session expired" }), {
				status: 401,
			}),
		);
		const { transport } = harness(fetch);
		await expect(transport.loadSnapshot()).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammerLifecycleHttpError>>({
				name: "ProgrammerLifecycleHttpError",
				message: "session expired",
				status: 401,
				retryable: false,
			}),
		);
	});

	it("subscribes to only the aggregate lifecycle object and repairs", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(8, observer);
		const socket = FakeWebSocket.instances[0];
		socket.emit("open");

		expect(String(socket.url)).toBe("ws://127.0.0.1:5000/api/v2/events");
		expect(socket.protocols).toEqual([
			"light.events.v2",
			"light.token.session-token",
			"light.desk.b64.ZGVzay1ib3VuZGFyeQ",
		]);
		expect(JSON.parse(socket.sent[0])).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["programmer"],
				classes: ["projection"],
				objects: [{ capability: "programmer", id: "programming-lifecycle" }],
			},
			after_sequence: 8,
			capacity: 128,
			rate_limits: [],
		});

		socket.emit("message", message(eventMessage()));
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 12 }),
		);
		stream.repair(12);
		expect(JSON.parse(socket.sent[1])).toEqual({
			type: "repair",
			cursor: { sequence: 12 },
		});
	});

	it("reports malformed traffic as a repairable protocol error", () => {
		const { observer, transport } = harness();
		transport.subscribe(null, observer);
		const malformed = eventMessage();
		malformed.event.delivery = "replaceable";
		FakeWebSocket.instances[0].emit("message", message(malformed));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<Partial<ProgrammerLifecycleProtocolError>>({
				name: "ProgrammerLifecycleProtocolError",
				eventSequence: 12,
				requiresRepair: true,
			}),
		);
	});

	it("suppresses close callbacks for explicit closure", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(null, observer);
		const socket = FakeWebSocket.instances[0];
		stream.close();
		socket.emit("close");
		expect(socket.close).toHaveBeenCalledOnce();
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
