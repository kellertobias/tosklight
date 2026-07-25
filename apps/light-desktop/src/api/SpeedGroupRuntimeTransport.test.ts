import { describe, expect, it, vi } from "vitest";
import type { SpeedGroupActionRequest } from "../features/speedGroupRuntime/contracts";
import {
	AUTHORITY_ID,
	CORRELATION_ID,
	DESK_ID,
	OTHER_DESK_ID,
	speedAuthority,
} from "../features/speedGroupRuntime/testFixtures";
import type { SpeedGroupTransportError } from "../features/speedGroupRuntime/transport";
import { HttpSpeedGroupRuntimeTransport } from "./SpeedGroupRuntimeTransport";

const SCOPE = { deskId: DESK_ID };
const REQUEST: SpeedGroupActionRequest = {
	requestId: "speed-request",
	expectedAuthorityId: AUTHORITY_ID,
	expectedRevision: 4,
	expectedGroups: speedAuthority().groups,
	action: { type: "set_bpm", group: "A", bpm: 128.5 },
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

function wireGroup(group: string, manualBpm = 120) {
	return {
		group,
		manual_bpm: manualBpm,
		paused: false,
		speed_master_scale: 1,
		synchronized_with: null,
		phase_origin_millis: 100,
	};
}

function wireGroups() {
	return ["A", "B", "C", "D", "E"].map((group) => wireGroup(group));
}

function wireSnapshot() {
	return {
		cursor: { sequence: 18 },
		projection: {
			authority_id: AUTHORITY_ID,
			revision: 4,
			groups: wireGroups(),
		},
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
			object: { capability: "playback", id: "speed-groups:manual" },
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "lossless",
			payload: {
				type: "speed_groups_changed",
				change: {
					authority_id: AUTHORITY_ID,
					revision: 5,
					applied_at_millis: 200,
					groups: [wireGroup("A", 128.5)],
				},
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
	const transport = new HttpSpeedGroupRuntimeTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		authenticatedDeskId,
		deskBoundaryToken: "desk-boundary",
		fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetch, observer, transport };
}

describe("HttpSpeedGroupRuntimeTransport", () => {
	it("constructs without snapshot, socket, bootstrap, or sound I/O", () => {
		const { fetch } = harness();
		expect(fetch).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("loads and writes only the authenticated desk's narrow v2 object", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(new Response(JSON.stringify(wireSnapshot())))
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						request_id: REQUEST.requestId,
						correlation_id: CORRELATION_ID,
						authority_id: AUTHORITY_ID,
						revision: 5,
						applied_at_millis: 200,
						groups: [wireGroup("A", 128.5)],
						status: "changed",
						event_sequence: 19,
						replayed: false,
						durability: "durable",
					}),
				),
			);
		const { transport } = harness(fetch);

		await expect(transport.loadSnapshot(SCOPE)).resolves.toMatchObject({
			cursor: 18,
			projection: { authorityId: AUTHORITY_ID },
		});
		await expect(transport.applyAction(SCOPE, REQUEST)).resolves.toMatchObject({
			status: "changed",
			eventSequence: 19,
		});
		const [getUrl, getOptions] = fetch.mock.calls[0] ?? [];
		const [postUrl, postOptions] = fetch.mock.calls[1] ?? [];
		expect(getUrl).toBe(
			`http://127.0.0.1:5000/api/v2/desks/${DESK_ID}/speed-groups`,
		);
		expect(postUrl).toBe(getUrl);
		expect(String(getUrl)).not.toMatch(/bootstrap|playbacks|sound/u);
		expect((getOptions?.headers as Headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect(postOptions?.method).toBe("POST");
		expect(JSON.parse(String(postOptions?.body))).toEqual({
			request_id: "speed-request",
			expected_authority_id: AUTHORITY_ID,
			expected_revision: 4,
			action: { type: "set_bpm", group: "A", bpm: 128.5 },
		});
	});

	it("maps typed conflicts and retryable network failures", async () => {
		const conflict = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
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
			harness(conflict).transport.applyAction(SCOPE, REQUEST),
		).rejects.toEqual(
			expect.objectContaining<Partial<SpeedGroupTransportError>>({
				name: "SpeedGroupTransportError",
				status: 409,
				currentRevision: 7,
			}),
		);
		const network = vi
			.fn<typeof globalThis.fetch>()
			.mockRejectedValue(new Error("connection reset"));
		await expect(
			harness(network).transport.loadSnapshot(SCOPE),
		).rejects.toMatchObject({ status: 0, retryable: true });
	});

	it("classifies malformed successful responses as repairable protocol errors", async () => {
		const malformed = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(new Response("{not-json"));
		await expect(
			harness(malformed).transport.loadSnapshot(SCOPE),
		).rejects.toMatchObject({
			name: "SpeedGroupProtocolError",
			requiresRepair: true,
		});
	});

	it("subscribes only to playback/speed-groups:manual and repairs by cursor", () => {
		const { observer, transport } = harness();
		const stream = transport.subscribe(SCOPE, 8, observer);
		const socket = FakeWebSocket.instances[0];
		if (!socket) throw new Error("missing socket");
		socket.emit("open");

		expect(String(socket.url)).toBe("ws://127.0.0.1:5000/api/v2/events");
		expect(JSON.parse(socket.sent[0] ?? "null")).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["playback"],
				classes: ["projection"],
				objects: [{ capability: "playback", id: "speed-groups:manual" }],
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

	it("rejects foreign desk scopes before HTTP or socket I/O", async () => {
		const { fetch, observer, transport } = harness();
		const foreign = { deskId: OTHER_DESK_ID };
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

	it("allows the same installation authority through two owning desks", async () => {
		const firstFetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(new Response(JSON.stringify(wireSnapshot())));
		const secondFetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(new Response(JSON.stringify(wireSnapshot())));
		const first = harness(firstFetch, DESK_ID).transport;
		const second = harness(secondFetch, OTHER_DESK_ID).transport;

		await expect(first.loadSnapshot(SCOPE)).resolves.toMatchObject({
			projection: { authorityId: AUTHORITY_ID },
		});
		await expect(
			second.loadSnapshot({ deskId: OTHER_DESK_ID }),
		).resolves.toMatchObject({ projection: { authorityId: AUTHORITY_ID } });
		await expect(first.loadSnapshot({ deskId: OTHER_DESK_ID })).rejects.toThrow(
			/authenticated desk/,
		);
	});

	it("rejects malformed or foreign-object socket traffic", () => {
		const { observer, transport } = harness();
		transport.subscribe(SCOPE, null, observer);
		const socket = FakeWebSocket.instances[0];
		if (!socket) throw new Error("missing socket");
		const foreign = eventMessage();
		foreign.event.object.id = "playback:1";
		socket.emit("message", message(foreign));
		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				name: "SpeedGroupProtocolError",
				eventSequence: 19,
			}),
		);
	});
});
