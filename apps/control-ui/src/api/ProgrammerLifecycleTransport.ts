import type { ProgrammerLifecycleSnapshot } from "../features/programmerLifecycle/contracts";
import type {
	ProgrammerLifecycleEventObserver,
	ProgrammerLifecycleEventStream,
	ProgrammerLifecycleEventTransport,
} from "../features/programmerLifecycle/transport";
import { ProgrammerLifecycleProtocolError } from "../features/programmerLifecycle/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeProgrammerLifecycleEventMessage,
	decodeProgrammerLifecycleSnapshot,
} from "./programmerLifecycleWire";

export interface ProgrammerLifecycleTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

export interface ProgrammerLifecycleTransport
	extends ProgrammerLifecycleEventTransport {
	loadSnapshot(): Promise<ProgrammerLifecycleSnapshot>;
}

export class ProgrammerLifecycleHttpError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerLifecycleHttpError";
	}
}

/** Authenticated aggregate adapter; it stays dormant until explicitly used. */
export class HttpProgrammerLifecycleTransport
	implements ProgrammerLifecycleTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: ProgrammerLifecycleTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(): Promise<ProgrammerLifecycleSnapshot> {
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/programmer-lifecycle/snapshot`,
			{ headers: this.headers() },
		);
		return decodeProgrammerLifecycleSnapshot(await responseValue(response));
	}

	subscribe(
		afterSequence: number | null,
		observer: ProgrammerLifecycleEventObserver,
	): ProgrammerLifecycleEventStream {
		validateSequence(afterSequence, "event cursor");
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		const lifecycle = { explicitlyClosed: false };
		bindSocket(socket, afterSequence, observer, lifecycle);
		return eventStream(socket, this.WebSocketImplementation.OPEN, lifecycle);
	}

	private eventUrl() {
		const url = new URL("/api/v2/events", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		return url;
	}

	private protocols() {
		const protocols = [
			"light.events.v2",
			`light.token.${this.options.sessionToken}`,
		];
		if (this.options.deskBoundaryToken)
			protocols.push(
				`light.desk.b64.${base64Url(this.options.deskBoundaryToken)}`,
			);
		return protocols;
	}

	private headers() {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}
}

interface SocketLifecycle {
	explicitlyClosed: boolean;
}

function bindSocket(
	socket: WebSocket,
	afterSequence: number | null,
	observer: ProgrammerLifecycleEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(subscription(afterSequence))),
	);
	socket.addEventListener("message", (event) => deliverEvent(event, observer));
	socket.addEventListener("error", () =>
		observer.error(new Error("Programmer lifecycle event connection failed")),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function deliverEvent(
	event: MessageEvent,
	observer: ProgrammerLifecycleEventObserver,
) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(decodeProgrammerLifecycleEventMessage(value));
	} catch (reason) {
		const error = reason instanceof Error ? reason : new Error(String(reason));
		observer.error(
			new ProgrammerLifecycleProtocolError(
				`Invalid Programmer lifecycle event: ${error.message}`,
				eventSequence(value),
			),
		);
	}
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): ProgrammerLifecycleEventStream {
	return {
		repair: (cursor) => repairStream(socket, cursor, openState),
		close: () => {
			lifecycle.explicitlyClosed = true;
			socket.close();
		},
	};
}

function repairStream(socket: WebSocket, cursor: number, openState: number) {
	validateSequence(cursor, "repair cursor");
	if (socket.readyState !== openState) return;
	socket.send(
		JSON.stringify({
			type: "repair",
			cursor: { sequence: cursor },
		} satisfies EventClientMessage),
	);
}

function subscription(afterSequence: number | null): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: ["programmer"],
			classes: ["projection"],
			objects: [{ capability: "programmer", id: "programming-lifecycle" }],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
}

function validateSequence(value: number | null, label: string) {
	if (value == null) return;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerLifecycleProtocolError(
			`Programmer lifecycle ${label} must be a non-negative safe integer`,
		);
}

async function responseValue(response: Response): Promise<unknown> {
	const text = await response.text();
	if (!response.ok) throw httpError(response, text);
	try {
		return text ? JSON.parse(text) : null;
	} catch {
		throw httpError(response, text || "response was not valid JSON");
	}
}

function httpError(response: Response, text: string) {
	let message = text || `${response.status} ${response.statusText}`;
	try {
		const parsed = JSON.parse(text) as { error?: unknown };
		if (typeof parsed.error === "string" && parsed.error)
			message = parsed.error;
	} catch {
		// Preserve the transport's plain-text failure.
	}
	return new ProgrammerLifecycleHttpError(
		message,
		response.status,
		response.status >= 500,
	);
}

function eventSequence(value: unknown): number | null {
	if (!value || typeof value !== "object" || Array.isArray(value)) return null;
	const event = (value as Record<string, unknown>).event;
	if (!event || typeof event !== "object" || Array.isArray(event)) return null;
	const sequence = (event as Record<string, unknown>).sequence;
	return Number.isSafeInteger(sequence) && (sequence as number) >= 0
		? (sequence as number)
		: null;
}

function base64Url(value: string) {
	const bytes = new TextEncoder().encode(value);
	let binary = "";
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary)
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replace(/=+$/, "");
}
