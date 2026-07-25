import { describe, expect, it, vi } from "vitest";
import type { ProgrammerCaptureModeProtocolError } from "../features/programmerCaptureMode/transport";
import {
	HttpProgrammerCaptureModeTransport,
	type ProgrammerCaptureModeHttpError,
} from "./ProgrammerCaptureModeTransport";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const SCOPE = { showId: SHOW_ID, userId: USER_ID };

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

function projection() {
	return {
		user_id: USER_ID,
		revision: 3,
		blind: true,
		preview: false,
		preload_capture_programmer: true,
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
			object: {
				capability: "programmer",
				id: `programming-capture-mode:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_capture_mode_changed",
				change: { projection: projection() },
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
	const transport = new HttpProgrammerCaptureModeTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpProgrammerCaptureModeTransport", () => {
	it("stays dormant until a snapshot or subscription is requested", () => {
		const { fetch } = harness();
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("loads and strictly decodes the authenticated user's narrow snapshot", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 11 },
					projection: projection(),
				}),
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.loadSnapshot(SCOPE)).resolves.toMatchObject({
			cursor: 11,
			projection: { userId: USER_ID, revision: 3 },
		});
		const [url, options] = fetch.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-capture-mode/snapshot`,
		);
		const headers = options?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("maps a typed HTTP failure without accepting its body as a snapshot", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					kind: "forbidden",
					error: "foreign Programmer user",
					current_revision: null,
					current_capture_mode_revision: null,
					retryable: false,
				}),
				{ status: 403 },
			),
		);
		const { transport } = harness(fetch);

		await expect(transport.loadSnapshot(SCOPE)).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammerCaptureModeHttpError>>({
				name: "ProgrammerCaptureModeHttpError",
				kind: "forbidden",
				status: 403,
				retryable: false,
			}),
		);
	});

	it("subscribes only to the exact user object and repairs from a cursor", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(SCOPE, 8, observer);
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
				objects: [
					{
						capability: "programmer",
						id: `programming-capture-mode:${USER_ID}`,
					},
				],
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
		transport.subscribe(SCOPE, null, observer);
		const malformed = eventMessage();
		(malformed.event as { desk_id: string | null }).desk_id = USER_ID;
		FakeWebSocket.instances[0].emit("message", message(malformed));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<Partial<ProgrammerCaptureModeProtocolError>>({
				name: "ProgrammerCaptureModeProtocolError",
				eventSequence: 12,
				requiresRepair: true,
			}),
		);
	});

	it("validates scope and suppresses close callbacks for explicit closure", () => {
		const { observer, transport } = harness();
		expect(() =>
			transport.subscribe({ showId: "show", userId: USER_ID }, null, observer),
		).toThrow();
		expect(FakeWebSocket.instances).toHaveLength(0);

		const stream = transport.subscribe(SCOPE, null, observer);
		const socket = FakeWebSocket.instances[0];
		stream.close();
		socket.emit("close");
		expect(socket.close).toHaveBeenCalledOnce();
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
