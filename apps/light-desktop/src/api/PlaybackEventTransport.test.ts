import { describe, expect, it, vi } from "vitest";
import { PlaybackRuntimeStore } from "../features/playbackRuntime/store";
import type { PlaybackProjection } from "../features/playbackRuntime/contracts";
import {
	CUE_LIST_ID,
	DESK_ID,
	GROUP_ID,
	SHOW_ID,
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
	it("routes settled feedback to its exact address and updates the internal store key", () => {
		const { observer, transport } = createHarness();
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID);
		const failed: Extract<PlaybackProjection, {target:"dynamic"}> = {
			scope: { show_id: SHOW_ID, show_revision: 4 },
			identity: { kind: "playback", page: 4, position: 904 },
			target: "dynamic",
			dynamic_id: CUE_LIST_ID,
			last_known_pool_number: 34,
			embedded: false,
			runtime: {
				owner_address: {page:4,position:904},
				enabled: true,
				paused: false,
				flash: false,
				activated_at: "2026-07-19T10:00:00Z",
				fader_value: 1,
				fader_pickup_required: false,
				fader_pickup_target: null,
				size: 1,
				master: 1,
				local_speed_numerator: 1,
				local_speed_denominator: 1,
				learned_duration_millis: null,
				state: "failed",
				instance_id: null,
				controller_id: CUE_LIST_ID,
				winning_controller_id: null,
				controller_status: "missing",
				target_count: 1,
				compatible_target_count: 1,
				missing_target_count: 0,
				unpatched_target_count: 0,
				lane_count: 1,
				supported_address_count: 1,
				skipped_address_count: 0,
				speed_source: "fixed",
				effective_speed_multiplier: 1,
				effective_duration_millis: 4000,
				warning: null,
			},
		};
		store.applyProjection(failed, 10);
		observer.message.mockImplementation((event) => {
			if (event.type === "event" && event.payload.type === "runtime")
				store.applyProjection(event.payload.projection, event.sequence);
		});
		transport.subscribe(DESK_ID, {
			identities: [failed.identity], desk: false, telemetry: false,
		}, 10, observer);
		const socket = FakeWebSocket.instances[0];
		socket.emit("open");
		expect(JSON.parse(socket.sent[0]).filter.objects).toEqual([
			{ capability: "playback", id: "playback:4.904" },
		]);
		const off = { ...failed, runtime: { ...failed.runtime!, state: "off", enabled: false } };
		socket.emit("message", message({
			type: "event",
			event: {
				sequence: 11,
				object: { capability: "playback", id: "playback:4.904" },
				payload: {
					type: "playback_runtime_changed",
					change: { projection: off, transition: null },
				},
			},
		}));
		expect(observer.error).not.toHaveBeenCalled();
		expect(store.getSnapshot().projections.get("playback:4.904")).toEqual([off]);
		store.applyProjection(failed, 10);
		expect(store.getSnapshot().projections.get("playback:4.904")).toEqual([off]);
	});
	it("subscribes only to mounted Playback identities and the desk view", () => {
		const { observer, transport } = createHarness();
		const stream = transport.subscribe(
			DESK_ID,
			{
				identities: [
					{ kind: "playback", page: 1, position: 2 },
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
				classes: ["transition", "projection", "telemetry"],
				objects: [
					{ capability: "playback", id: "playback:1.2" },
					{ capability: "playback", id: `cuelist:${CUE_LIST_ID}` },
					{ capability: "playback", id: `group:${GROUP_ID}` },
					{ capability: "playback", id: "telemetry" },
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

	it("omits sampled telemetry for a warm-cache-only subscription", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			DESK_ID,
			{
				identities: [{ kind: "playback", page: 1, position: 2 }],
				desk: true,
				telemetry: false,
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
					{ capability: "playback", id: "playback:1.2" },
					{ capability: "desk", id: `playback-view:${DESK_ID}` },
				],
			},
			after_sequence: 8,
			capacity: 128,
			rate_limits: [],
		});
	});

	it("decodes a validated Playback projection and ignores another capability", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			DESK_ID,
			{ identities: [{ kind: "playback", page: 1, position: 1 }], desk: false },
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
			{ identities: [{ kind: "playback", page: 1, position: 1 }], desk: false },
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
			{ identities: [{ kind: "playback", page: 1, position: 1 }], desk: false },
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
