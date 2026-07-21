import { describe, expect, it, vi } from "vitest";
import {
	CUE_LIST_ID,
	DESK_ID,
	GROUP_ID,
	runtimeEvent,
} from "../features/playbackRuntime/testFixtures";
import type { PlaybackProtocolError } from "../features/playbackRuntime/transport";
import { WebSocketPlaybackEventTransport } from "./PlaybackEventTransport";

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

function message(value: unknown) {
	return { data: JSON.stringify(value) } as MessageEvent;
}

function createHarness() {
	FakeWebSocket.instances = [];
	const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
	const transport = new WebSocketPlaybackEventTransport({
		baseUrl: "http://127.0.0.1:5000",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { observer, transport };
}

describe("WebSocketPlaybackEventTransport", () => {
	it("subscribes only to mounted Playback identities and the desk view", () => {
		const { observer, transport } = createHarness();
		const stream = transport.subscribe(
			DESK_ID,
			{
				identities: [
					{ kind: "playback", playback_number: 2 },
					{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
					{ kind: "group", group_id: GROUP_ID },
				],
				desk: true,
			},
			8,
			observer,
		);
		const socket = FakeWebSocket.instances[0];
		socket.emit("open");
		expect(JSON.parse(socket.sent[0])).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["playback", "desk"],
				classes: ["transition", "projection"],
				objects: [
					{ capability: "playback", id: "playback:2" },
					{ capability: "playback", id: `cuelist:${CUE_LIST_ID}` },
					{ capability: "playback", id: `group:${GROUP_ID}` },
					{ capability: "desk", id: `playback-view:${DESK_ID}` },
				],
			},
			after_sequence: 8,
			capacity: 128,
			rate_limits: [],
		});
		stream.repair(12);
		expect(JSON.parse(socket.sent[1])).toEqual({
			type: "repair",
			cursor: { sequence: 12 },
		});
	});

	it("decodes a validated Playback projection and ignores another capability", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			DESK_ID,
			{ identities: [{ kind: "playback", playback_number: 1 }], desk: false },
			0,
			observer,
		);
		const socket = FakeWebSocket.instances[0];
		socket.emit("message", message(runtimeEvent()));
		socket.emit(
			"message",
			message({
				type: "event",
				event: {
					sequence: 12,
					payload: { type: "output_runtime_changed", change: {} },
				},
			}),
		);
		expect(observer.message).toHaveBeenCalledOnce();
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 11 }),
		);
	});

	it("rejects a malformed routed projection without advancing it", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			DESK_ID,
			{ identities: [{ kind: "playback", playback_number: 1 }], desk: false },
			0,
			observer,
		);
		const malformed = runtimeEvent();
		const projection = malformed.event.payload.change.projection;
		if (projection.target !== "cue_list" || !projection.runtime)
			throw new Error("fixture must contain a running Cuelist");
		projection.runtime.paused = "yes" as unknown as boolean;
		FakeWebSocket.instances[0].emit("message", message(malformed));
		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<Partial<PlaybackProtocolError>>({
				name: "PlaybackProtocolError",
				eventSequence: 11,
			}),
		);
	});

	it("rejects malformed event routing and envelope sequences", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			DESK_ID,
			{ identities: [{ kind: "playback", playback_number: 1 }], desk: false },
			0,
			observer,
		);
		const malformedRoute = runtimeEvent();
		malformedRoute.event.object.capability = "database";
		FakeWebSocket.instances[0].emit("message", message(malformedRoute));
		const malformedSequence = runtimeEvent();
		malformedSequence.event.sequence = "eleven" as unknown as number;
		FakeWebSocket.instances[0].emit("message", message(malformedSequence));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledTimes(2);
		expect(observer.error).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({ eventSequence: 11 }),
		);
		expect(observer.error).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ eventSequence: null }),
		);
	});
});
