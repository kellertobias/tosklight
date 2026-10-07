import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VisualizationRuntimeScope } from "../features/visualizationRuntime/contracts";
import { HttpVisualizationRuntimeTransport } from "./VisualizationRuntimeTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";
const FIXTURE = "33333333-3333-4333-8333-333333333333";
const scope: VisualizationRuntimeScope = {
	showId: SHOW_ID,
	sessionId: SESSION_ID,
	authorityKey: "server-a",
};

class FakeWebSocket extends EventTarget {
	static readonly OPEN = 1;
	static instances: FakeWebSocket[] = [];
	readonly sent: string[] = [];
	readyState = 0;

	constructor(readonly url: string | URL) {
		super();
		FakeWebSocket.instances.push(this);
	}

	open() {
		this.readyState = FakeWebSocket.OPEN;
		this.dispatchEvent(new Event("open"));
	}

	message(value: unknown) {
		this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(value) }));
	}

	send(value: string) {
		this.sent.push(value);
	}

	close() {
		this.readyState = 3;
		this.dispatchEvent(new Event("close"));
	}
}

function openStream() {
	FakeWebSocket.instances = [];
	const observer = { snapshot: vi.fn(), error: vi.fn(), readouts: vi.fn() };
	const stream = new HttpVisualizationRuntimeTransport({
		baseUrl: "http://desk.test/",
		sessionToken: "session-token",
		showId: SHOW_ID,
		sessionId: SESSION_ID,
		authorityKey: "server-a",
		fetch: vi.fn<typeof globalThis.fetch>(),
		webSocket: FakeWebSocket as unknown as typeof WebSocket,
	}).openStream(scope, observer);
	stream.updateClaims(["normal"], 10);
	const socket = FakeWebSocket.instances[0] as FakeWebSocket;
	socket.open();
	return { stream, socket, observer };
}

function readouts(sequence: number) {
	return {
		type: "readouts",
		sequence,
		source_frame: 41,
		readouts: {
			lane: "normal",
			scope: { show_id: SHOW_ID },
			lease: 9,
			revision: 3,
			owners: [
				{
					fixture_id: FIXTURE,
					position: {
						available: true,
						commands: [],
						common: { pan_degrees: 12, tilt_degrees: -4 },
					},
				},
			],
		},
	};
}

const sent = (socket: FakeWebSocket) =>
	socket.sent.map((message) => JSON.parse(message) as Record<string, unknown>);

describe("visualization stream readouts (TL-594)", () => {
	beforeEach(() => {
		vi.spyOn(Date, "now").mockReturnValue(Date.parse("2026-07-21T09:00:00.050Z"));
	});
	afterEach(() => vi.restoreAllMocks());

	it("sends the readout claim with Subscribe and replaces or clears it", () => {
		const { stream, socket } = openStream();
		expect(sent(socket).at(-1)).not.toHaveProperty("readouts");

		stream.updateReadoutClaim?.([FIXTURE, FIXTURE]);
		expect(sent(socket).at(-1)).toMatchObject({
			type: "subscribe",
			lanes: ["normal"],
			readouts: { fixture_ids: [FIXTURE, FIXTURE] },
		});
		const count = socket.sent.length;
		stream.updateReadoutClaim?.([FIXTURE, FIXTURE]);
		expect(socket.sent).toHaveLength(count);

		stream.updateReadoutClaim?.(null);
		expect(sent(socket).at(-1)).toMatchObject({ type: "subscribe" });
		expect(sent(socket).at(-1)).not.toHaveProperty("readouts");
	});

	it("accepts a readouts message and hands the decoded snapshot to the observer", () => {
		const { socket, observer } = openStream();

		socket.message(readouts(1));
		socket.message([readouts(2)]);

		expect(observer.error).not.toHaveBeenCalled();
		expect(observer.readouts).toHaveBeenCalledTimes(2);
		expect(observer.readouts.mock.calls[0]?.[0]).toMatchObject({
			lane: "normal",
			lease: 9,
			owners: [{ fixture_id: FIXTURE }],
		});
		expect(observer.readouts.mock.calls[0]?.[1]).toBe(41);
		expect(socket.readyState).toBe(FakeWebSocket.OPEN);
		expect(sent(socket).some((message) => message.type === "resynchronize")).toBe(false);
	});

	it("still rejects unknown message types and readouts of another Show", () => {
		const unknown = openStream();
		unknown.socket.message({ type: "sideways", sequence: 1 });
		expect(unknown.observer.error).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Visualization stream message type sideways is unsupported",
			}),
		);
		expect(unknown.socket.readyState).toBe(3);

		const foreign = openStream();
		const message = readouts(1);
		message.readouts.scope.show_id = "44444444-4444-4444-8444-444444444444";
		foreign.socket.message(message);
		expect(foreign.observer.readouts).not.toHaveBeenCalled();
		expect(foreign.observer.error).toHaveBeenCalled();
	});
});
