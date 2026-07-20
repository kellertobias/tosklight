import type {
	ProgrammerPreloadPlaybackQueueScope,
	ProgrammerPreloadPlaybackQueueSnapshot,
} from "../features/programmerPreloadPlaybackQueue/contracts";
import type {
	ProgrammerPreloadPlaybackQueueEventObserver,
	ProgrammerPreloadPlaybackQueueEventStream,
	ProgrammerPreloadPlaybackQueueEventTransport,
} from "../features/programmerPreloadPlaybackQueue/transport";
import { ProgrammerPreloadPlaybackQueueProtocolError } from "../features/programmerPreloadPlaybackQueue/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeProgrammerPreloadPlaybackQueueEventMessage,
	decodeProgrammerPreloadPlaybackQueueSnapshot,
} from "./programmerPreloadPlaybackQueueWire";
import { programmerPreloadValuesUuidAt } from "./programmerPreloadValuesWireProjection";

export interface ProgrammerPreloadPlaybackQueueTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedUserId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

export interface ProgrammerPreloadPlaybackQueueTransport
	extends ProgrammerPreloadPlaybackQueueEventTransport {
	loadSnapshot(
		scope: ProgrammerPreloadPlaybackQueueScope,
	): Promise<ProgrammerPreloadPlaybackQueueSnapshot>;
}

export class ProgrammerPreloadPlaybackQueueHttpError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerPreloadPlaybackQueueHttpError";
	}
}

/** Exact-user read adapter; construction performs no I/O. */
export class HttpProgrammerPreloadPlaybackQueueTransport
	implements ProgrammerPreloadPlaybackQueueTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: ProgrammerPreloadPlaybackQueueTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(
		scope: ProgrammerPreloadPlaybackQueueScope,
	): Promise<ProgrammerPreloadPlaybackQueueSnapshot> {
		this.validateScope(scope);
		const response = await this.fetchImplementation(
			`${this.queuePath(scope)}/snapshot`,
			{ headers: this.headers() },
		);
		return decodeProgrammerPreloadPlaybackQueueSnapshot(
			await responseValue(response),
			scope.userId,
		);
	}

	subscribe(
		scope: ProgrammerPreloadPlaybackQueueScope,
		afterSequence: number | null,
		observer: ProgrammerPreloadPlaybackQueueEventObserver,
	): ProgrammerPreloadPlaybackQueueEventStream {
		this.validateScope(scope);
		validateSequence(afterSequence, "event cursor");
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		const lifecycle = { explicitlyClosed: false };
		bindSocket(socket, scope.userId, afterSequence, observer, lifecycle);
		return eventStream(socket, this.WebSocketImplementation.OPEN, lifecycle);
	}

	private validateScope(scope: ProgrammerPreloadPlaybackQueueScope) {
		programmerPreloadValuesUuidAt(scope.showId, "$.scope.showId");
		const userId = programmerPreloadValuesUuidAt(
			scope.userId,
			"$.scope.userId",
		);
		const authenticated = programmerPreloadValuesUuidAt(
			this.options.authenticatedUserId,
			"$.authenticatedUserId",
		);
		if (userId.toLowerCase() !== authenticated.toLowerCase())
			throw new ProgrammerPreloadPlaybackQueueProtocolError(
				"Preload playback queue scope does not match the authenticated user",
			);
	}

	private queuePath(scope: ProgrammerPreloadPlaybackQueueScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-preload-playback-queue`;
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
	userId: string,
	afterSequence: number | null,
	observer: ProgrammerPreloadPlaybackQueueEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(subscription(userId, afterSequence))),
	);
	socket.addEventListener("message", (event) =>
		deliverEvent(event, userId, observer),
	);
	socket.addEventListener("error", () =>
		observer.error(new Error("Preload playback queue event connection failed")),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function deliverEvent(
	event: MessageEvent,
	userId: string,
	observer: ProgrammerPreloadPlaybackQueueEventObserver,
) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(
			decodeProgrammerPreloadPlaybackQueueEventMessage(value, userId),
		);
	} catch (reason) {
		const error = reason instanceof Error ? reason : new Error(String(reason));
		observer.error(
			new ProgrammerPreloadPlaybackQueueProtocolError(
				`Invalid Preload playback queue event: ${error.message}`,
				eventSequence(value),
			),
		);
	}
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): ProgrammerPreloadPlaybackQueueEventStream {
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

function subscription(
	userId: string,
	afterSequence: number | null,
): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: ["programmer"],
			classes: ["projection"],
			objects: [
				{
					capability: "programmer",
					id: `programming-preload-playback-queue:${userId}`,
				},
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
}

function validateSequence(value: number | null, label: string) {
	if (value == null) return;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerPreloadPlaybackQueueProtocolError(
			`Preload playback queue ${label} must be a non-negative safe integer`,
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
		// Preserve a plain-text transport failure.
	}
	return new ProgrammerPreloadPlaybackQueueHttpError(
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
