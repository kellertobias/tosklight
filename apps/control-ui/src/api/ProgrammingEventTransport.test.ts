import { describe, expect, it, vi } from "vitest";
import { ProgrammingProtocolError } from "../features/programmingInteraction/transport";
import type { ProgrammingEventScope } from "../features/programmingInteraction/transport";
import { WebSocketProgrammingEventTransport } from "./ProgrammingEventTransport";
import {
	PROGRAMMING_DESK_ID,
	programmingEvent,
} from "./programmingWireTestFixtures";

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
	const transport = new WebSocketProgrammingEventTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	});
	return { observer, transport };
}

function expectedObjects(scope: ProgrammingEventScope) {
	return [
		...(scope.commandLine
			? [
					{
						capability: "desk",
						id: `programming-command-line:${PROGRAMMING_DESK_ID}`,
					},
				]
			: []),
		...(scope.selection
			? [
					{
						capability: "desk",
						id: `programming-selection:${PROGRAMMING_DESK_ID}`,
					},
				]
			: []),
	];
}

describe("WebSocketProgrammingEventTransport", () => {
	it.each([
		{ commandLine: true, selection: false },
		{ commandLine: false, selection: true },
		{ commandLine: true, selection: true },
	])("subscribes only to the mounted $scope views", (scope) => {
		const { observer, transport } = createHarness();
		const stream = transport.subscribe(
			PROGRAMMING_DESK_ID,
			scope,
			8,
			observer,
		);
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
				capabilities: ["desk"],
				classes: ["projection"],
				objects: expectedObjects(scope),
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

	it("decodes a scoped sparse event and ignores another capability", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			PROGRAMMING_DESK_ID,
			{ commandLine: true, selection: false },
			20,
			observer,
		);
		const socket = FakeWebSocket.instances[0];
		socket.emit("message", message(programmingEvent("commandLine")));
		socket.emit(
			"message",
			message({
				type: "event",
				event: {
					sequence: 22,
					payload: { type: "output_runtime_changed" },
				},
			}),
		);

		expect(observer.message).toHaveBeenCalledOnce();
		expect(observer.message).toHaveBeenCalledWith(
			expect.objectContaining({ type: "event", sequence: 21 }),
		);
		expect(observer.error).not.toHaveBeenCalled();
	});

	it("reports malformed or out-of-scope events without advancing them", () => {
		const { observer, transport } = createHarness();
		transport.subscribe(
			PROGRAMMING_DESK_ID,
			{ commandLine: false, selection: true },
			20,
			observer,
		);
		const malformed = programmingEvent("selection");
		malformed.event.delivery = "replaceable";
		const outsideScope = programmingEvent("commandLine");
		FakeWebSocket.instances[0].emit("message", message(malformed));
		FakeWebSocket.instances[0].emit("message", message(outsideScope));

		expect(observer.message).not.toHaveBeenCalled();
		expect(observer.error).toHaveBeenCalledTimes(2);
		for (const [error] of observer.error.mock.calls)
			expect(error).toEqual(
				expect.objectContaining<Partial<ProgrammingProtocolError>>({
					name: "ProgrammingProtocolError",
					eventSequence: 21,
				}),
			);
	});

	it("rejects an empty or malformed subscription before opening a socket", () => {
		const { observer, transport } = createHarness();
		expect(() =>
			transport.subscribe(
				PROGRAMMING_DESK_ID,
				{ commandLine: false, selection: false },
				null,
				observer,
			),
		).toThrow(ProgrammingProtocolError);
		expect(() =>
			transport.subscribe(
				"desk-1",
				{ commandLine: true, selection: false },
				null,
				observer,
			),
		).toThrow(ProgrammingProtocolError);
		expect(() =>
			transport.subscribe(
				PROGRAMMING_DESK_ID,
				{ commandLine: true, selection: false },
				-1,
				observer,
			),
		).toThrow(ProgrammingProtocolError);
		expect(FakeWebSocket.instances).toHaveLength(0);
	});

	it("does not report an explicitly closed stream as a connection loss", () => {
		const { observer, transport } = createHarness();
		const stream = transport.subscribe(
			PROGRAMMING_DESK_ID,
			{ commandLine: true, selection: false },
			null,
			observer,
		);
		const socket = FakeWebSocket.instances[0];
		stream.close();
		socket.emit("close");
		expect(socket.close).toHaveBeenCalledOnce();
		expect(observer.closed).not.toHaveBeenCalled();
	});
});
