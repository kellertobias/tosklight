import { describe, expect, it, vi } from "vitest";
import {
	CORRELATION_ID,
	DESK_ID,
	OTHER_DESK_ID,
	OTHER_SHOW_ID,
	SHOW_ID,
} from "../features/outputRuntime/testFixtures";
import type { OutputRuntimeTransportError } from "../features/outputRuntime/transport";
import { HttpOutputRuntimeTransport } from "./OutputRuntimeTransport";

const SCOPE = { showId: SHOW_ID, deskId: DESK_ID };
const REQUEST = {
	requestId: "output-request",
	expectedShowId: SHOW_ID,
	expectedRevision: 4,
	grandMaster: 0.4,
	blackout: true,
};

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

function wireProjection(overrides: Record<string, unknown> = {}) {
	return {
		scope: { show_id: SHOW_ID },
		identity: "global_master",
		revision: 5,
		grand_master: 0.4,
		blackout: true,
		...overrides,
	};
}

function eventMessage(projection = wireProjection()) {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-21T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: { capability: "output", id: "runtime:global-master" },
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "output_runtime_changed",
				change: { projection },
			},
		},
	};
}

function message(value: unknown) {
	return { data: JSON.stringify(value) } as MessageEvent;
}

function harness(
	fetch = vi.fn<typeof globalThis.fetch>(),
	authenticatedDeskId = DESK_ID,
) {
	FakeWebSocket.instances = [];
	const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
	const transport = new HttpOutputRuntimeTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		authenticatedDeskId,
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpOutputRuntimeTransport", () => {
	it("constructs without snapshot, socket, bootstrap, or visualization I/O", () => {
		const { fetch } = harness();
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("loads only the desk-authenticated narrow global projection", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 18 },
					projection: wireProjection(),
				}),
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.loadSnapshot(SCOPE)).resolves.toMatchObject({
			cursor: 18,
			projection: { showId: SHOW_ID, grandMaster: 0.4, blackout: true },
		});
		const [url, options] = fetch.mock.calls[0] ?? [];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/desks/${DESK_ID}/output-runtime/global-master`,
		);
		expect(String(url)).not.toMatch(/bootstrap|visualization|playbacks/u);
		const headers = options?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("posts one strict combined action with authority headers", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: wireProjection(),
					status: "changed",
					event_sequence: 19,
					replayed: false,
					durability: "durable",
				}),
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.applyAction(SCOPE, REQUEST)).resolves.toMatchObject({
			status: "changed",
			eventSequence: 19,
			durability: "durable",
		});
		const [url, options] = fetch.mock.calls[0] ?? [];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/desks/${DESK_ID}/output-runtime/global-master`,
		);
		expect(options?.method).toBe("POST");
		expect(JSON.parse(String(options?.body))).toEqual({
			request_id: "output-request",
			expected_show_id: SHOW_ID,
			expected_revision: 4,
			grand_master: 0.4,
			blackout: true,
		});
		expect((options?.headers as Headers).get("content-type")).toBe(
			"application/json",
		);
	});

	it("maps typed conflicts and network failures for repair or replay", async () => {
		const conflictFetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					kind: "conflict",
					error: "revision conflict",
					current_revision: 7,
					retryable: false,
				}),
				{ status: 409 },
			),
		);
		await expect(
			harness(conflictFetch).transport.applyAction(SCOPE, REQUEST),
		).rejects.toEqual(
			expect.objectContaining<Partial<OutputRuntimeTransportError>>({
				name: "OutputRuntimeTransportError",
				status: 409,
				currentRevision: 7,
				retryable: false,
			}),
		);
		const networkFetch = vi
			.fn<typeof globalThis.fetch>()
			.mockRejectedValue(new Error("connection reset"));
		await expect(
			harness(networkFetch).transport.loadSnapshot(SCOPE),
		).rejects.toMatchObject({
			name: "OutputRuntimeTransportError",
			status: 0,
			retryable: true,
		});
	});

	it("subscribes only to the global output object and repairs by cursor", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(SCOPE, 8, observer);
		const socket = FakeWebSocket.instances[0];
		if (!socket) throw new Error("missing socket");
		socket.emit("open");

		expect(String(socket.url)).toBe("ws://127.0.0.1:5000/api/v2/events");
		expect(socket.protocols).toEqual([
			"light.events.v2",
			"light.token.session-token",
			"light.desk.b64.ZGVzay1ib3VuZGFyeQ",
		]);
		expect(JSON.parse(socket.sent[0] ?? "null")).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["output"],
				classes: ["projection"],
				objects: [{ capability: "output", id: "runtime:global-master" }],
			},
			after_sequence: 8,
			capacity: 128,
			rate_limits: [],
		});

		socket.emit("message", message(eventMessage()));
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 19 }),
		);
		stream.repair(19);
		expect(JSON.parse(socket.sent[1] ?? "null")).toEqual({
			type: "repair",
			cursor: { sequence: 19 },
		});
	});

	it("rejects foreign desk scopes before any HTTP or socket I/O", async () => {
		const { fetch, observer, transport } = harness();
		const foreign = { showId: SHOW_ID, deskId: OTHER_DESK_ID };

		await expect(transport.loadSnapshot(foreign)).rejects.toThrow(
			/authenticated desk/,
		);
		await expect(transport.applyAction(foreign, REQUEST)).rejects.toThrow(
			/authenticated desk/,
		);
		expect(() => transport.subscribe(foreign, null, observer)).toThrow(
			/authenticated desk/,
		);
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("allows the same installation projection through each owning desk only", async () => {
		const firstFetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 18 },
					projection: wireProjection(),
				}),
			),
		);
		const secondFetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 18 },
					projection: wireProjection(),
				}),
			),
		);
		const first = harness(firstFetch, DESK_ID).transport;
		const second = harness(secondFetch, OTHER_DESK_ID).transport;

		await expect(first.loadSnapshot(SCOPE)).resolves.toMatchObject({
			projection: { grandMaster: 0.4 },
		});
		await expect(
			second.loadSnapshot({ showId: SHOW_ID, deskId: OTHER_DESK_ID }),
		).resolves.toMatchObject({ projection: { grandMaster: 0.4 } });
		await expect(
			first.loadSnapshot({ showId: SHOW_ID, deskId: OTHER_DESK_ID }),
		).rejects.toThrow(/authenticated desk/);
		await expect(second.loadSnapshot(SCOPE)).rejects.toThrow(
			/authenticated desk/,
		);
	});

	it("rejects foreign-Show socket traffic and suppresses explicit close", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(SCOPE, null, observer);
		const socket = FakeWebSocket.instances[0];
		if (!socket) throw new Error("missing socket");
		const foreign = eventMessage(
			wireProjection({ scope: { show_id: OTHER_SHOW_ID } }),
		);
		socket.emit("message", message(foreign));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				name: "OutputRuntimeProtocolError",
				eventSequence: 19,
			}),
		);
		stream.close();
		socket.emit("close");
		expect(socket.close).toHaveBeenCalledOnce();
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
