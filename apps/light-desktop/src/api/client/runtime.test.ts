import {
	afterEach,
	beforeEach,
	describe,
	expect,
	expectTypeOf,
	it,
	vi,
} from "vitest";
import type { EventPayload } from "../generated/light-wire";
import type { BootstrapSnapshot, SessionResponse } from "../types";
import { LightClientRuntime } from "./runtime";
import type { LiveClientTransport } from "./transport";

class FakeWebSocket {
	static readonly OPEN = 1;
	static instances: FakeWebSocket[] = [];

	readonly sent: string[] = [];
	readonly readyState = FakeWebSocket.OPEN;
	onclose: ((event: Event) => void) | null = null;
	private readonly listeners = new Map<
		string,
		Array<(event: Event | MessageEvent) => void>
	>();

	constructor(
		readonly url: string | URL,
		readonly protocols: string[],
	) {
		FakeWebSocket.instances.push(this);
	}

	addEventListener(
		type: string,
		listener: EventListenerOrEventListenerObject,
	): void {
		const callback =
			typeof listener === "function"
				? listener
				: (event: Event) => listener.handleEvent(event);
		const listeners = this.listeners.get(type) ?? [];
		listeners.push(callback);
		this.listeners.set(type, listeners);
	}

	close(): void {
		this.emit("close");
	}

	send(value: string): void {
		this.sent.push(value);
	}

	emit(type: string, event: Event | MessageEvent = new Event(type)): void {
		for (const listener of this.listeners.get(type) ?? []) listener(event);
		if (type === "close") this.onclose?.(event);
	}

	emitMessage(data: unknown): void {
		this.emit("message", { data: JSON.stringify(data) } as MessageEvent);
	}
}

beforeEach(() => {
	FakeWebSocket.instances = [];
	vi.stubGlobal("WebSocket", FakeWebSocket);
	vi.stubGlobal("localStorage", memoryStorage());
	vi.stubGlobal("sessionStorage", memoryStorage());
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe("LightClientRuntime", () => {
	it("preserves the public runtime method contracts", () => {
		expectTypeOf<ReturnType<LightClientRuntime["bootstrap"]>>().toEqualTypeOf<
			Promise<BootstrapSnapshot>
		>();
		expectTypeOf<
			ReturnType<LightClientRuntime["capabilityTransport"]>
		>().toEqualTypeOf<LiveClientTransport>();
		expectTypeOf<ReturnType<LightClientRuntime["onEvent"]>>().toEqualTypeOf<
			() => boolean
		>();
	});

	it("opens the established event endpoint with ordered UTF-8 credentials", async () => {
		const client = connectedClient();
		client.setDeskToken("ä desk");
		const onClose = vi.fn();

		const connecting = client.connectEvents(onClose);
		const socket = FakeWebSocket.instances[0];
		expect(String(socket.url)).toBe("ws://desk.local/api/v2/events");
		expect(socket.protocols).toEqual([
			"light.events.v2",
			"light.token.token-a",
			"light.desk.b64.w6QgZGVzaw",
		]);
		socket.emit("open");
		await connecting;
		expect(JSON.parse(socket.sent[0])).toEqual({
			type: "subscribe",
			filter: { capabilities: ["system", "output", "desk", "show"] },
			capacity: 256,
			rate_limits: [],
		});

		client.disconnectEvents();
		expect(onClose).not.toHaveBeenCalled();
	});

	it("dispatches events until the listener unsubscribes", async () => {
		const client = connectedClient();
		const listener = vi.fn();
		const unsubscribe = client.onEvent(listener);
		const socket = await openEvents(client);
		const event = {
			type: "server_configuration_changed",
			change: { revision: 4 },
		} satisfies EventPayload;

		socket.emitMessage(typedEvent(event));
		expect(listener).toHaveBeenCalledWith(event);
		expect(unsubscribe()).toBe(true);
		expect(unsubscribe()).toBe(false);
		socket.emitMessage(
			typedEvent({
				type: "server_configuration_changed",
				change: { revision: 5 },
			}),
		);
		expect(listener).toHaveBeenCalledOnce();
	});

	it("delivers typed volatile Macro and Timecode runtime projections", async () => {
		const client = connectedClient();
		const listener = vi.fn();
		client.onEvent(listener);
		const socket = await openEvents(client);
		const macro = {
			type: "macro_execution_changed",
			execution: {
				execution_id: "execution-a",
				macro_id: "macro-a",
				macro_number: 7,
				macro_name: "Blackout",
				source_revision: 2,
				desk_id: "desk-a",
				user_id: "user-a",
				session_id: "session-a",
				state: "succeeded",
				trigger: { type: "pool" },
				started_at: "2026-08-10T18:00:00Z",
				finished_at: "2026-08-10T18:00:01Z",
			},
		} satisfies EventPayload;
		const timecode = {
			type: "timecode_runtime_changed",
			snapshot: {
				timecode_id: "timecode-a",
				revision: 12,
				state: "playing",
				frame: 88,
				duration_frame: 440,
				audio_linked: true,
			},
		} satisfies EventPayload;

		socket.emitMessage(typedEvent(macro));
		socket.emitMessage(typedEvent(timecode));

		expect(listener).toHaveBeenNthCalledWith(1, macro);
		expect(listener).toHaveBeenNthCalledWith(2, timecode);
	});

	it("maps typed operator notifications without a generic facade payload", async () => {
		const client = connectedClient();
		const listener = vi.fn();
		client.onEvent(listener);
		const socket = await openEvents(client);

		const event = {
			type: "operator_notification",
			notification: {
				type: "desk_action",
				revision: 7,
				notification: {
					action: "clear",
					control: null,
					value: null,
					session_id: "session-a",
					desk_id: "desk-a",
					desk_alias: "main",
				},
			},
		} satisfies EventPayload;
		socket.emitMessage(typedEvent(event));

		expect(listener).toHaveBeenCalledWith(event);
	});

	it("forwards authoritative typed Highlight state", async () => {
		const client = connectedClient();
		const listener = vi.fn();
		client.onEvent(listener);
		const socket = await openEvents(client);
		const state = {
			active: true,
			mode: "selection",
			output_enabled: true,
			capture_only: false,
			remembered: [],
			active_index: null,
			active_fixture: null,
			can_previous: false,
			can_next: false,
			owner_user_id: "user-a",
			owner_user_name: "Operator",
			message: null,
		};

		const event = {
			type: "highlight_changed",
			change: {
				revision: 9,
				desk_id: "desk-a",
				user_id: "user-a",
				action: "on",
				source: null,
				state,
			},
		} satisfies EventPayload;
		socket.emitMessage(typedEvent(event));

		expect(listener).toHaveBeenCalledWith(event);
	});

	it("correlates action responses and rejects actions after the timeout", async () => {
		vi.useFakeTimers();
		vi.spyOn(crypto, "randomUUID")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000001")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000002");
		const client = connectedClient();
		const socket = await openEvents(client);

		const command = client
			.capabilityTransport()
			.sendAction({ type: "programmer_undo" });
		expect(JSON.parse(socket.sent[1])).toEqual({
			type: "action",
			protocol_version: 2,
			request_id: "00000000-0000-4000-8000-000000000001",
			session_id: "session-a",
			action: { type: "programmer_undo" },
		});
		socket.emitMessage({
			protocol_version: 2,
			request_id: "00000000-0000-4000-8000-000000000001",
			ok: true,
			revision: 8,
			payload: { revision: 8 },
		});
		await expect(command).resolves.toEqual({ revision: 8 });

		const timedOut = expect(
			client.capabilityTransport().sendAction({ type: "programmer_undo" }),
		).rejects.toThrow("Action timed out: programmer_undo");
		await vi.advanceTimersByTimeAsync(5_000);
		await timedOut;
	});

	it("never replays an unresolved command onto a reconnected socket", async () => {
		vi.useFakeTimers();
		const client = connectedClient();
		const first = await openEvents(client);
		const unresolved = expect(
			client.capabilityTransport().sendAction(
				{
					type: "playback",
					request: {
						request_id: "playback-a",
						address: { kind: "playback", playback_number: 1 },
						action: { type: "go", pressed: true },
						surface: "physical",
					},
				},
				"playback-a",
			),
		).rejects.toThrow("Action timed out: playback");
		expect(first.sent).toHaveLength(2);

		const reconnecting = client.connectEvents();
		const replacement = FakeWebSocket.instances.at(-1);
		if (!replacement) throw new Error("Expected a replacement event socket");
		replacement.emit("open");
		await reconnecting;

		expect(replacement).not.toBe(first);
		expect(replacement.sent.map((message) => JSON.parse(message))).toEqual([
			{
				type: "subscribe",
				filter: {
					capabilities: ["system", "output", "desk", "show"],
				},
				capacity: 256,
				rate_limits: [],
			},
		]);
		await vi.advanceTimersByTimeAsync(5_000);
		await unresolved;
		expect(replacement.sent).toHaveLength(1);
	});

	it("closes a gapped typed subscription so connection bootstrap repairs state", async () => {
		const client = connectedClient();
		const onClose = vi.fn();
		const connecting = client.connectEvents(onClose);
		const socket = FakeWebSocket.instances.at(-1);
		if (!socket) throw new Error("Expected an event socket");
		socket.emit("open");
		await connecting;

		socket.emitMessage({
			type: "gap",
			gap: {
				after_sequence: 1,
				oldest_available: 3,
				latest_sequence: 4,
			},
		});
		expect(onClose).toHaveBeenCalledOnce();
	});
});

function typedEvent(payload: EventPayload) {
	return {
		type: "event",
		event: {
			payload,
		},
	};
}

async function openEvents(client: LightClientRuntime): Promise<FakeWebSocket> {
	const connecting = client.connectEvents();
	const socket = FakeWebSocket.instances.at(-1);
	if (!socket) throw new Error("Expected an event socket");
	socket.emit("open");
	await connecting;
	return socket;
}

function connectedClient(): LightClientRuntime {
	const client = new LightClientRuntime("http://desk.local");
	client.restoreSession(session());
	return client;
}

function session(): SessionResponse {
	return {
		session_id: "session-a",
		client_id: "client-a",
		token: "token-a",
		user: { id: "user-a", name: "Operator", enabled: true },
		desk: {
			id: "desk-a",
			name: "Main",
			osc_alias: "main",
			columns: 10,
			rows: 4,
			buttons: 40,
		},
	};
}

function memoryStorage(): Storage {
	const values = new Map<string, string>();
	return {
		get length() {
			return values.size;
		},
		clear: () => values.clear(),
		getItem: (key) => values.get(key) ?? null,
		key: (index) => [...values.keys()][index] ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	};
}
