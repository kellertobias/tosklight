import { describe, expect, it, vi } from "vitest";
import {
	CORRELATION_ID,
	OTHER_USER_ID,
	USER_ID,
} from "../features/programmerPriority/testFixtures";
import type { ProgrammerPriorityTransportError } from "../features/programmerPriority/transport";
import { HttpProgrammerPriorityTransport } from "./ProgrammerPriorityTransport";

const SCOPE = { userId: USER_ID };
const REQUEST = {
	requestId: "priority-request",
	expectedRevision: 4,
	priority: 8,
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
		user_id: USER_ID,
		revision: 5,
		priority: 8,
		changed_at: "2026-07-21T10:00:00Z",
		...overrides,
	};
}

function eventMessage() {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-21T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-priority:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programmer_priority_changed",
				change: { type: "upsert", projection: wireProjection() },
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
	const transport = new HttpProgrammerPriorityTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		authenticatedUserId: USER_ID,
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpProgrammerPriorityTransport", () => {
	it("constructs without a snapshot, socket, or broad bootstrap request", () => {
		const { fetch } = harness();
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("loads only the authenticated user's narrow snapshot", async () => {
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
			projection: { userId: USER_ID, priority: 8 },
		});
		const [url, options] = fetch.mock.calls[0] ?? [];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-priority/snapshot`,
		);
		expect(String(url)).not.toContain("bootstrap");
		const headers = options?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("posts one strict typed action with authority headers", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: wireProjection(),
					status: "changed",
					event_sequence: 19,
					replayed: false,
				}),
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.applyAction(SCOPE, REQUEST)).resolves.toMatchObject({
			status: "changed",
			eventSequence: 19,
		});
		const [url, options] = fetch.mock.calls[0] ?? [];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-priority/actions`,
		);
		expect(options?.method).toBe("POST");
		expect(JSON.parse(String(options?.body))).toEqual({
			request_id: "priority-request",
			expected_revision: 4,
			priority: 8,
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
			expect.objectContaining<Partial<ProgrammerPriorityTransportError>>({
				name: "ProgrammerPriorityTransportError",
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
			name: "ProgrammerPriorityTransportError",
			status: 0,
			retryable: true,
		});
	});

	it("subscribes only to the exact user object and repairs from a cursor", () => {
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
				capabilities: ["programmer"],
				classes: ["projection"],
				objects: [
					{
						capability: "programmer",
						id: `programming-priority:${USER_ID}`,
					},
				],
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

	it("rejects foreign snapshot, action, and subscription scopes before I/O", async () => {
		const { fetch, observer, transport } = harness();
		const foreign = { userId: OTHER_USER_ID };

		await expect(transport.loadSnapshot(foreign)).rejects.toThrow(
			/authenticated user/,
		);
		await expect(transport.applyAction(foreign, REQUEST)).rejects.toThrow(
			/authenticated user/,
		);
		expect(() => transport.subscribe(foreign, null, observer)).toThrow(
			/authenticated user/,
		);
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("rejects foreign socket traffic and suppresses explicit close callbacks", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(SCOPE, null, observer);
		const socket = FakeWebSocket.instances[0];
		if (!socket) throw new Error("missing socket");
		const foreign = eventMessage();
		foreign.event.object.id = `programming-priority:${OTHER_USER_ID}`;
		socket.emit("message", message(foreign));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				name: "ProgrammerPriorityProtocolError",
				eventSequence: 19,
			}),
		);
		stream.close();
		socket.emit("close");
		expect(socket.close).toHaveBeenCalledOnce();
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
