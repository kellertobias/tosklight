import { DeskSnapshotStore } from "../../features/deskSnapshot/store";
import { createServerEventRouter } from "../../features/server/serverEventRouter";
import type { ServerState } from "../../features/server/useServerState";
import {
	afterEach,
	beforeEach,
	describe,
	expect,
	expectTypeOf,
	it,
	vi,
} from "vitest";
import {
	commandErrorFeedback,
	reportProgrammingCommandError,
} from "../../features/programmingInteraction/commandError";
import { ApiRequestError } from "../ApiRequestError";
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
	it("contains malformed clock projections without stranding shared listeners or stale lock", async () => {
		const client = connectedClient();
		const store = new DeskSnapshotStore();
		const session = client.currentSession;
		if (!session) throw new Error("Connected client must have its session");
		const state = {
			session,
			deskSnapshotStore: store,
			mediaServers: [],
		} as unknown as ServerState;
		client.onEvent(createServerEventRouter(() => state, session, vi.fn()));
		const later = vi.fn();
		client.onEvent(later);
		const socket = await openEvents(client);
		const snapshot = {
			revision: 5,
			selected_source: "artnet:127.0.0.1:2",
			active_source: "artnet:127.0.0.1:2",
			state: "locked",
			current: "00:00:02:22",
			source_rate: "25",
			desk_rate: "44",
			rate_warning: null,
		};
		const event = (next: unknown) =>
			typedEvent({
				type: "timecode_clock_changed",
				snapshot: next,
			} as EventPayload);
		socket.emitMessage(event(snapshot));
		expect(store.getSnapshot().clock?.current).toBe(snapshot.current);
		expect(() =>
			socket.emitMessage(event({ ...snapshot, revision: 6, current: 123 })),
		).not.toThrow();
		expect(later).toHaveBeenCalledTimes(2);
		expect(store.getSnapshot()).toMatchObject({
			clock: null,
			clockError: "Timecode status unavailable",
		});
		socket.emitMessage(
			event({ ...snapshot, revision: 4, current: "00:00:01:00" }),
		);
		expect(store.getSnapshot().clock).toBeNull();
		socket.emitMessage(
			event({ ...snapshot, revision: 7, current: "00:00:03:00" }),
		);
		expect(store.getSnapshot().clock?.current).toBe("00:00:03:00");
		expect(store.getSnapshot().clockError).toBeNull();
		client.disconnectEvents();
	});

	it.each([
		["Update would not change Group 1 1", "UPDATE GROUP 1"],
		["Update would not change Front wash front-wash", "UPDATE GROUP 1"],
		[
			"Update would not change Cuelist 5 6b53ba0e-099c-4656-b309-1311c53484b6",
			"UPDATE PBK 3.5",
		],
		[
			"Playback 3.5 has no current Cue; identify an explicit Cue",
			"UPDATE PBK 3.5",
		],
		["OFFSET must be smaller than DIV", "GROUP 100 DIV 2 OFFSET 2"],
		["offset is invalid", "GROUP 100 DIV 2 OFFSET INVALID"],
		["+ requires an offset", "GROUP 100 DIV 2 +"],
		[
			"targetless Dynamics cannot be assigned directly to a Playback",
			"ASSIGN DYNAMIC 31 AT PBK 3.8",
		],
	])("keeps operator rejection local after its actual socket response: %s", async (message, command) => {
		const client = connectedClient();
		const socket = await openEvents(client);
		const dispatch = vi.spyOn(window, "dispatchEvent");
		const outcome = client
			.capabilityTransport()
			.sendAction({
				type: "command_line_execute",
				request: { value: command },
			})
			.catch((error: Error) => reportProgrammingCommandError(error));
		const frame = JSON.parse(socket.sent[1]);
		socket.emitMessage({
			protocol_version: 2,
			request_id: frame.request_id,
			ok: false,
			revision: 8,
			error: message,
		});
		await outcome;
		expect(
			(dispatch.mock.lastCall?.[0] as CustomEvent | undefined)?.detail,
		).toEqual({
			message,
			kind: "operator",
		});
		// OSC history uses the same plain server text, without an HTTP status.
		expect(commandErrorFeedback(message).kind).toBe("operator");
		client.disconnectEvents();
		dispatch.mockRestore();
	});
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

	it("turns a successful HTML startup response into an actionable error", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn().mockResolvedValue(
				new Response("<html>wrong server</html>", {
					status: 200,
					headers: { "content-type": "text/html" },
				}),
			),
		);

		const error = await new LightClientRuntime("http://desk.local")
			.bootstrap()
			.catch((reason: unknown) => reason);

		expect(error).toEqual(
			new ApiRequestError(
				"ToskLight server returned an invalid response. Check that the server address points to a running ToskLight server.",
				200,
			),
		);
		expect(String(error)).not.toMatch(/Unexpected token|wrong server/u);
	});

	it("turns malformed JSON startup responses into an actionable error", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn().mockResolvedValue(
				new Response("{", {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
			),
		);

		await expect(
			new LightClientRuntime("http://desk.local").bootstrap(),
		).rejects.toEqual(
			new ApiRequestError(
				"ToskLight server returned an invalid response. Check that the server address points to a running ToskLight server.",
				200,
			),
		);
	});

	it("reports the server's error envelope as its human message, not raw JSON", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn().mockResolvedValue(
				new Response(
					JSON.stringify({
						kind: "not_found",
						error: "Group not found",
						retryable: false,
					}),
					{ status: 404, headers: { "content-type": "application/json" } },
				),
			),
		);

		const error = await new LightClientRuntime("http://desk.local")
			.bootstrap()
			.catch((reason: unknown) => reason);

		expect(error).toEqual(new ApiRequestError("Group not found", 404));
		expect(String(error)).not.toMatch(/[{}]|not_found|retryable/u);
	});

	it("keeps a plain-text error body unchanged", async () => {
		vi.stubGlobal(
			"fetch",
			vi
				.fn()
				.mockResolvedValue(
					new Response("the active show is changing", { status: 409 }),
				),
		);

		await expect(
			new LightClientRuntime("http://desk.local").bootstrap(),
		).rejects.toEqual(new ApiRequestError("the active show is changing", 409));
	});

	it("accepts JSON response media types with parameters", async () => {
		vi.stubGlobal(
			"fetch",
			vi.fn().mockResolvedValue(
				new Response('{"ready":true}', {
					status: 200,
					headers: { "content-type": "application/json; charset=utf-8" },
				}),
			),
		);

		await expect(
			new LightClientRuntime("http://desk.local")
				.capabilityTransport()
				.request<{ ready: boolean }>("/api/v2/bootstrap", {}, false),
		).resolves.toEqual({ ready: true });
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
		socket.emitMessage({ type: "ready", cursor: { sequence: 1 } });
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

	it("waits for the server subscription boundary before reporting connected", async () => {
		const client = connectedClient();
		let connected = false;
		const connecting = client.connectEvents().then(() => {
			connected = true;
		});
		const socket = FakeWebSocket.instances.at(-1)!;
		socket.emit("open");
		await Promise.resolve();
		await Promise.resolve();
		expect(connected).toBe(false);
		socket.emitMessage({ type: "ready", cursor: { sequence: 1 } });
		await connecting;
		expect(connected).toBe(true);
	});

	it("rejects a connection closed before its subscription is ready", async () => {
		const client = connectedClient();
		const connecting = expect(client.connectEvents()).rejects.toThrow(
			"WebSocket closed before its subscription was ready",
		);
		const socket = FakeWebSocket.instances.at(-1)!;
		socket.emit("open");
		socket.close();
		await connecting;
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
				external_armed: false,
				revision: 12,
				state: "playing",
				frame: 88,
				duration_frame: 440,
				audio_linked: true,
				cue_list_clips: [],
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
					path: "main",
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
						address: { kind: "playback", page: 1, position: 1 },
						action: { type: "go", pressed: true },
						control_source: {
							surface_id: "software:test-client",
							control_id: 11,
						},
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
		replacement.emitMessage({ type: "ready", cursor: { sequence: 1 } });
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
		socket.emitMessage({ type: "ready", cursor: { sequence: 1 } });
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
	socket.emitMessage({ type: "ready", cursor: { sequence: 1 } });
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
		desk: {
			id: "desk-a",
			name: "Main",
			surface_id: "main",
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
