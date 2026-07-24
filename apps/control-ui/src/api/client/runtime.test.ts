import { afterEach, beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import { LightApiClient } from "../LightApiClient";
import type { BootstrapSnapshot, ServerEvent, SessionResponse } from "../types";

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
		expectTypeOf<ReturnType<LightApiClient["bootstrap"]>>().toEqualTypeOf<
			Promise<BootstrapSnapshot>
		>();
		expectTypeOf<ReturnType<LightApiClient["command"]>>().toEqualTypeOf<
			Promise<unknown>
		>();
		expectTypeOf<ReturnType<LightApiClient["onEvent"]>>().toEqualTypeOf<
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
			filter: { capabilities: ["system"] },
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
		const event: ServerEvent = {
			revision: 4,
			kind: "programmer_changed",
			payload: { session_id: "session-a" },
		};

		socket.emitMessage(facadeEvent(event));
		expect(listener).toHaveBeenCalledWith(event);
		expect(unsubscribe()).toBe(true);
		expect(unsubscribe()).toBe(false);
		socket.emitMessage(facadeEvent({ ...event, revision: 5 }));
		expect(listener).toHaveBeenCalledOnce();
	});

	it("correlates command responses and rejects commands after the timeout", async () => {
		vi.useFakeTimers();
		vi.spyOn(crypto, "randomUUID")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000001")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000002");
		const client = connectedClient();
		const socket = await openEvents(client);

		const command = client.command("programmer.clear", {});
		expect(JSON.parse(socket.sent[1])).toEqual({
			protocol_version: 1,
			request_id: "00000000-0000-4000-8000-000000000001",
			session_id: "session-a",
			command: "programmer.clear",
			payload: {},
		});
		socket.emitMessage({
			protocol_version: 1,
			request_id: "00000000-0000-4000-8000-000000000001",
			ok: true,
			revision: 8,
			payload: { revision: 8 },
		});
		await expect(command).resolves.toEqual({ revision: 8 });

		const timedOut = expect(
			client.command("programmer.undo", {}),
		).rejects.toThrow("Command timed out: programmer.undo");
		await vi.advanceTimersByTimeAsync(5_000);
		await timedOut;
	});

	it("never replays an unresolved command onto a reconnected socket", async () => {
		vi.useFakeTimers();
		const client = connectedClient();
		const first = await openEvents(client);
		const unresolved = expect(
			client.command("playback.action", { request_id: "playback-a" }),
		).rejects.toThrow("Command timed out: playback.action");
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
				filter: { capabilities: ["system"] },
				capacity: 256,
				rate_limits: [],
			},
		]);
		await vi.advanceTimersByTimeAsync(5_000);
		await unresolved;
		expect(replacement.sent).toHaveLength(1);
	});

	it("closes a gapped facade subscription so connection bootstrap repairs state", async () => {
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

function facadeEvent(notification: ServerEvent) {
	return {
		type: "event",
		event: {
			payload: {
				type: "facade_notification",
				notification,
			},
		},
	};
}

async function openEvents(client: LightApiClient): Promise<FakeWebSocket> {
	const connecting = client.connectEvents();
	const socket = FakeWebSocket.instances.at(-1);
	if (!socket) throw new Error("Expected an event socket");
	socket.emit("open");
	await connecting;
	return socket;
}

function connectedClient(): LightApiClient {
	const client = new LightApiClient("http://desk.local");
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
