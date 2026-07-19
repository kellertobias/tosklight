import { describe, expect, it, vi } from "vitest";
import { WebSocketShowObjectsEventTransport } from "./ShowObjectsEventTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

class FakeWebSocket {
	static readonly OPEN = 1;
	readonly sent: string[] = [];
	readonly close = vi.fn();
	private readonly listeners = new Map<string, Array<(event: Event) => void>>();

	constructor(
		readonly url: string | URL,
		readonly protocols: string[],
	) {
		FakeWebSocket.instances.push(this);
	}

	static instances: FakeWebSocket[] = [];

	addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
		const callback =
			typeof listener === "function"
				? listener
				: (event: Event) => listener.handleEvent(event);
		const listeners = this.listeners.get(type) ?? [];
		listeners.push(callback);
		this.listeners.set(type, listeners);
	}

	send(value: string) {
		this.sent.push(value);
	}

	emit(type: string, event: Event = new Event(type)) {
		for (const listener of this.listeners.get(type) ?? []) listener(event);
	}
}

function eventMessage(data: unknown) {
	return { data: JSON.stringify(data) } as MessageEvent;
}

describe("WebSocketShowObjectsEventTransport", () => {
	it("subscribes to only the active show's lossless projection object", () => {
		FakeWebSocket.instances = [];
		const observer = {
			message: vi.fn(),
			error: vi.fn(),
			closed: vi.fn(),
		};
		const transport = new WebSocketShowObjectsEventTransport({
			baseUrl: "http://127.0.0.1:5000",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-boundary",
			webSocket: FakeWebSocket as unknown as typeof WebSocket,
		});

		const stream = transport.subscribe(SHOW_ID, 41, observer);
		const socket = FakeWebSocket.instances[0];
		expect(String(socket.url)).toBe("ws://127.0.0.1:5000/api/v2/events");
		expect(socket.protocols.slice(0, 2)).toEqual([
			"light.events.v2",
			"light.token.session-token",
		]);
		socket.emit("open");
		expect(JSON.parse(socket.sent[0])).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["show"],
				classes: ["projection"],
				objects: [{ capability: "show", id: `objects:${SHOW_ID}` }],
			},
			after_sequence: 41,
			capacity: 128,
			rate_limits: [],
		});

		stream.close();
		expect(socket.close).toHaveBeenCalledOnce();
		socket.emit("close");
		expect(observer.closed).not.toHaveBeenCalled();
	});

	it("maps exact Group/Preset bodies and ignores unrelated object kinds", () => {
		FakeWebSocket.instances = [];
		const observer = {
			message: vi.fn(),
			error: vi.fn(),
			closed: vi.fn(),
		};
		const transport = new WebSocketShowObjectsEventTransport({
			baseUrl: "https://desk.example.test",
			sessionToken: "token",
			webSocket: FakeWebSocket as unknown as typeof WebSocket,
		});
		transport.subscribe(SHOW_ID, null, observer);
		const socket = FakeWebSocket.instances[0];
		socket.emit(
			"message",
			eventMessage({
				type: "event",
				event: {
					sequence: 52,
					payload: {
						type: "show_objects_changed",
						change: {
							show_id: SHOW_ID,
							show_revision: 14,
							changes: [
								{
									kind: "group",
									object_id: "1",
									object_revision: 3,
									body: { name: "Front", fixtures: ["fixture-1"] },
									deleted: false,
								},
								{
									kind: "cue_list",
									object_id: "cues",
									object_revision: 2,
									body: { name: "Main" },
									deleted: false,
								},
							],
						},
					},
				},
			}),
		);

		expect(observer.error).not.toHaveBeenCalled();
		expect(observer.message).toHaveBeenCalledWith({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 14,
				eventSequence: 52,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 3,
						body: { name: "Front", fixtures: ["fixture-1"] },
						deleted: false,
					},
				],
			},
		});
	});
});
