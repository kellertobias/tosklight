import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPreloadPlaybackQueueProtocolError } from "../features/programmerPreloadPlaybackQueue/transport";
import { HttpProgrammerPreloadPlaybackQueueTransport } from "./ProgrammerPreloadPlaybackQueueTransport";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const OTHER_USER = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const scope = { showId: SHOW_ID, userId: USER_ID };

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
		revision: 2,
		actions: [{ playback_number: 4, page: 5, action: "go", surface: "osc" }],
	};
}

function eventMessage() {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-preload-playback-queue:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "osc" },
			correlation_id: null,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_playback_queue_changed",
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
	const transport = new HttpProgrammerPreloadPlaybackQueueTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		authenticatedUserId: USER_ID,
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpProgrammerPreloadPlaybackQueueTransport", () => {
	it("stays dormant until an exact-user snapshot is requested", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					cursor: { sequence: 11 },
					projection: projection(),
				}),
			),
		);
		const { transport } = harness(fetch);
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);

		await expect(transport.loadSnapshot(scope)).resolves.toMatchObject({
			cursor: 11,
			projection: {
				userId: USER_ID,
				revision: 2,
				actions: [{ playbackNumber: 4, page: 5 }],
			},
		});
		const [url, init] = fetch.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-preload-playback-queue/snapshot`,
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("rejects a foreign scope before any network request", async () => {
		const { fetch, transport } = harness();
		await expect(
			transport.loadSnapshot({ showId: SHOW_ID, userId: OTHER_USER }),
		).rejects.toThrow(/authenticated user/);
		expect(fetch).not.toHaveBeenCalled();
		expect(() =>
			transport.subscribe({ showId: SHOW_ID, userId: OTHER_USER }, null, {
				message: vi.fn(),
				error: vi.fn(),
				closed: vi.fn(),
			}),
		).toThrow(/authenticated user/);
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("subscribes only to the exact queue object and repairs", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(scope, 8, observer);
		const socket = FakeWebSocket.instances[0];
		socket.emit("open");

		expect(JSON.parse(socket.sent[0])).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["programmer"],
				classes: ["projection"],
				objects: [
					{
						capability: "programmer",
						id: `programming-preload-playback-queue:${USER_ID}`,
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
		transport.subscribe(scope, null, observer);
		const malformed = eventMessage();
		malformed.event.delivery = "lossless";
		FakeWebSocket.instances[0].emit("message", message(malformed));
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<
				Partial<ProgrammerPreloadPlaybackQueueProtocolError>
			>({ eventSequence: 12, requiresRepair: true }),
		);
	});

	it("maps authenticated HTTP errors without accepting them as snapshots", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(JSON.stringify({ error: "session expired" }), {
				status: 401,
			}),
		);
		const { transport } = harness(fetch);
		await expect(transport.loadSnapshot(scope)).rejects.toMatchObject({
			name: "ProgrammerPreloadPlaybackQueueHttpError",
			message: "session expired",
			status: 401,
			retryable: false,
		});
	});
});
