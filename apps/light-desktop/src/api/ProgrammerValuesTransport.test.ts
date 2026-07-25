import { describe, expect, it, vi } from "vitest";
import type { ProgrammerValuesProtocolError } from "../features/programmerValues/transport";
import {
	HttpProgrammerValuesTransport,
	type ProgrammerValuesActionError,
} from "./ProgrammerValuesTransport";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const OTHER_USER_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const FIXTURE_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
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

function projection(userId = USER_ID) {
	return {
		user_id: userId,
		revision: 3,
		fixture_values: [
			{
				fixture_id: FIXTURE_ID,
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
				programmer_order: 1,
				fade: false,
			},
		],
		group_values: [],
	};
}

function changedOutcome() {
	return {
		request_id: "request-1",
		correlation_id: CORRELATION_ID,
		revision: 3,
		capture_mode_revision: 4,
		status: "changed",
		projection: projection(),
		event_sequence: 12,
		replayed: false,
		warning: null,
	};
}

function event(userId = USER_ID) {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-19T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-values:${userId}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_values_changed",
				change: { projection: projection(userId) },
			},
		},
	};
}

function jsonResponse(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}

function message(value: unknown) {
	return { data: JSON.stringify(value) } as MessageEvent;
}

function createHarness(fetchImplementation = vi.fn()) {
	FakeWebSocket.instances = [];
	const transport = new HttpProgrammerValuesTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch: fetchImplementation as typeof fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetchImplementation, transport };
}

describe("HttpProgrammerValuesTransport HTTP", () => {
	it("is dormant until an exact-user snapshot is requested", async () => {
		const { fetchImplementation, transport } = createHarness();
		expect(fetchImplementation).not.toHaveBeenCalled();
		expect(FakeWebSocket.instances).toHaveLength(0);
		fetchImplementation.mockResolvedValueOnce(
			jsonResponse({ cursor: { sequence: 11 }, projection: projection() }),
		);

		await expect(transport.loadSnapshot(scope)).resolves.toMatchObject({
			cursor: 11,
			projection: { userId: USER_ID, revision: 3 },
		});
		expect(fetchImplementation).toHaveBeenCalledOnce();
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-values/snapshot`,
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("posts one batch request and validates the matching authoritative outcome", async () => {
		const { fetchImplementation, transport } = createHarness();
		fetchImplementation.mockResolvedValueOnce(jsonResponse(changedOutcome()));
		const request = {
			requestId: "request-1",
			expectedRevision: 2,
			expectedCaptureModeRevision: 4,
			action: {
				action: "batch" as const,
				mutations: [
					{
						action: "set_fixture" as const,
						fixtureId: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized" as const, value: 0.5 },
						timing: { fade: false, fadeMillis: null, delayMillis: null },
					},
				],
			},
		};

		await expect(transport.applyAction(scope, request)).resolves.toMatchObject({
			status: "changed",
			requestId: "request-1",
			revision: 3,
			captureModeRevision: 4,
			eventSequence: 12,
		});
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-values/actions`,
		);
		expect(init?.method).toBe("POST");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "request-1",
			expected_revision: 2,
			expected_capture_mode_revision: 4,
			action: {
				type: "batch",
				mutations: [
					{
						type: "set_fixture",
						fixture_id: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.5 },
						timing: {
							fade: false,
							fade_millis: null,
							delay_millis: null,
						},
					},
				],
			},
		});
	});

	it("surfaces typed revision conflicts and rejects a foreign snapshot", async () => {
		const { fetchImplementation, transport } = createHarness();
		fetchImplementation.mockResolvedValueOnce(
			jsonResponse(
				{
					kind: "conflict",
					error: "revision conflict",
					current_revision: 4,
					current_capture_mode_revision: 6,
					retryable: false,
				},
				409,
			),
		);
		await expect(
			transport.applyAction(scope, {
				requestId: "request-1",
				expectedRevision: 2,
				expectedCaptureModeRevision: 5,
				action: { action: "clear" },
			}),
		).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammerValuesActionError>>({
				kind: "conflict",
				status: 409,
				currentRevision: 4,
				currentCaptureModeRevision: 6,
				retryable: false,
			}),
		);
		fetchImplementation.mockResolvedValueOnce(
			jsonResponse({
				cursor: { sequence: 11 },
				projection: projection(OTHER_USER_ID),
			}),
		);
		await expect(transport.loadSnapshot(scope)).rejects.toThrow(
			/requested user/,
		);
	});
});

describe("HttpProgrammerValuesTransport events", () => {
	it("subscribes only to the current user's replaceable projection object", () => {
		const { transport } = createHarness();
		const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
		const stream = transport.subscribe(scope, 10, observer);
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
						id: `programming-values:${USER_ID}`,
					},
				],
			},
			after_sequence: 10,
			capacity: 128,
			rate_limits: [],
		});
		socket.emit("message", message(event()));
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 12 }),
		);
		stream.repair(12);
		expect(JSON.parse(socket.sent[1])).toEqual({
			type: "repair",
			cursor: { sequence: 12 },
		});
	});

	it("reports a foreign event as a repair-requiring protocol error", () => {
		const { transport } = createHarness();
		const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
		transport.subscribe(scope, null, observer);
		FakeWebSocket.instances[0].emit("message", message(event(OTHER_USER_ID)));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<Partial<ProgrammerValuesProtocolError>>({
				name: "ProgrammerValuesProtocolError",
				eventSequence: 12,
				requiresRepair: true,
			}),
		);
	});

	it("rejects foreign-user subscription shapes before opening a socket", () => {
		const { transport } = createHarness();
		const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
		expect(() =>
			transport.subscribe(
				{ showId: SHOW_ID, userId: "foreign-user" },
				null,
				observer,
			),
		).toThrow(/UUID/);
		expect(FakeWebSocket.instances).toHaveLength(0);
	});
});
