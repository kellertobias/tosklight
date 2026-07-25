import { describe, expect, it, vi } from "vitest";
import type { ProgrammerPreloadValuesProtocolError } from "../features/programmerPreloadValues/transport";
import {
	HttpProgrammerPreloadValuesTransport,
	type ProgrammerPreloadValuesActionError,
} from "./ProgrammerPreloadValuesTransport";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const OTHER_USER_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const FIXTURE_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const scope = { showId: SHOW_ID, userId: USER_ID };
const foreignScope = { showId: SHOW_ID, userId: OTHER_USER_ID };

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

function changedOutcome(userId = USER_ID) {
	return {
		request_id: "request-1",
		correlation_id: CORRELATION_ID,
		revision: 3,
		capture_mode_revision: 4,
		status: "changed",
		projection: projection(userId),
		event_sequence: 12,
		replayed: false,
		warning: null,
	};
}

function preloadEvent(userId = USER_ID) {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-preload-values:${userId}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_values_changed",
				change: { projection: projection(userId) },
			},
		},
	};
}

function request() {
	return {
		requestId: "request-1",
		expectedPreloadRevision: 2,
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
	const transport = new HttpProgrammerPreloadValuesTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		authenticatedUserId: USER_ID,
		deskBoundaryToken: "desk-boundary",
		fetch: fetchImplementation as typeof fetch,
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { fetchImplementation, transport };
}

describe("HttpProgrammerPreloadValuesTransport HTTP", () => {
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
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-preload-values/snapshot`,
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-boundary");
	});

	it("posts one ordered batch and maps its authoritative outcome", async () => {
		const { fetchImplementation, transport } = createHarness();
		fetchImplementation.mockResolvedValueOnce(jsonResponse(changedOutcome()));

		await expect(
			transport.applyAction(scope, request()),
		).resolves.toMatchObject({
			status: "changed",
			requestId: "request-1",
			preloadRevision: 3,
			captureModeRevision: 4,
			eventSequence: 12,
		});
		const [url, init] = fetchImplementation.mock.calls[0];
		expect(url).toBe(
			`http://127.0.0.1:5000/api/v2/users/${USER_ID}/programmer-preload-values/actions`,
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

	it("surfaces typed Preload and capture revision conflicts", async () => {
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

		await expect(transport.applyAction(scope, request())).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammerPreloadValuesActionError>>({
				kind: "conflict",
				status: 409,
				currentPreloadRevision: 4,
				currentCaptureModeRevision: 6,
				retryable: false,
			}),
		);
	});

	it("rejects foreign snapshot and action projections", async () => {
		const { fetchImplementation, transport } = createHarness();
		fetchImplementation.mockResolvedValueOnce(
			jsonResponse({
				cursor: { sequence: 11 },
				projection: projection(OTHER_USER_ID),
			}),
		);
		await expect(transport.loadSnapshot(scope)).rejects.toThrow(
			/requested user/,
		);
		fetchImplementation.mockResolvedValueOnce(
			jsonResponse(changedOutcome(OTHER_USER_ID)),
		);
		await expect(transport.applyAction(scope, request())).rejects.toThrow(
			/requested user/,
		);
	});

	it("rejects every foreign-user HTTP scope before making a request", async () => {
		const { fetchImplementation, transport } = createHarness();

		await expect(transport.loadSnapshot(foreignScope)).rejects.toThrow(
			/authenticated user/,
		);
		await expect(
			transport.applyAction(foreignScope, request()),
		).rejects.toThrow(/authenticated user/);
		expect(fetchImplementation).not.toHaveBeenCalled();
	});
});

describe("HttpProgrammerPreloadValuesTransport events", () => {
	it("subscribes only to the authenticated user's Preload projection object", () => {
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
		expect(JSON.parse(socket.sent[0] ?? "")).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["programmer"],
				classes: ["projection"],
				objects: [
					{
						capability: "programmer",
						id: `programming-preload-values:${USER_ID}`,
					},
				],
			},
			after_sequence: 10,
			capacity: 128,
			rate_limits: [],
		});
		socket.emit("message", message(preloadEvent()));
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 12 }),
		);
		stream.repair(12);
		expect(JSON.parse(socket.sent[1] ?? "")).toEqual({
			type: "repair",
			cursor: { sequence: 12 },
		});
	});

	it("reports foreign events as repair-requiring protocol errors", () => {
		const { transport } = createHarness();
		const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };
		transport.subscribe(scope, null, observer);
		FakeWebSocket.instances[0]?.emit(
			"message",
			message(preloadEvent(OTHER_USER_ID)),
		);

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledWith(
			expect.objectContaining<Partial<ProgrammerPreloadValuesProtocolError>>({
				name: "ProgrammerPreloadValuesProtocolError",
				eventSequence: 12,
				requiresRepair: true,
			}),
		);
	});

	it("rejects a different valid user before opening a socket", () => {
		const { transport } = createHarness();
		const observer = { message: vi.fn(), error: vi.fn(), closed: vi.fn() };

		expect(() => transport.subscribe(foreignScope, null, observer)).toThrow(
			/authenticated user/,
		);
		expect(FakeWebSocket.instances).toHaveLength(0);
	});
});
