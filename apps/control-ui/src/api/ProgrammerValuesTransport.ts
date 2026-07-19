import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesScope,
	ProgrammerValuesSnapshot,
} from "../features/programmerValues/contracts";
import type {
	ProgrammerValuesEventObserver,
	ProgrammerValuesEventStream,
	ProgrammerValuesEventTransport,
} from "../features/programmerValues/transport";
import { ProgrammerValuesProtocolError } from "../features/programmerValues/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeProgrammerValuesActionOutcome,
	decodeProgrammerValuesErrorResponse,
	decodeProgrammerValuesEventMessage,
	decodeProgrammerValuesSnapshot,
	encodeProgrammerValuesActionRequest,
	type ProgrammerValuesErrorKind,
} from "./programmerValuesWire";
import { programmerValuesUuidAt } from "./programmerValuesWireProjection";

export interface HttpProgrammerValuesTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

export interface ProgrammerValuesTransport
	extends ProgrammerValuesEventTransport {
	loadSnapshot(scope: ProgrammerValuesScope): Promise<ProgrammerValuesSnapshot>;
	applyAction(
		scope: ProgrammerValuesScope,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome>;
}

export class ProgrammerValuesActionError extends Error {
	constructor(
		message: string,
		readonly kind: ProgrammerValuesErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly currentCaptureModeRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerValuesActionError";
	}
}

/** Dormant until a caller explicitly loads, mutates, or subscribes a values view. */
export class HttpProgrammerValuesTransport
	implements ProgrammerValuesTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: HttpProgrammerValuesTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(
		scope: ProgrammerValuesScope,
	): Promise<ProgrammerValuesSnapshot> {
		validateScope(scope);
		const response = await this.fetchImplementation(
			`${this.valuesPath(scope)}/snapshot`,
			{ headers: this.headers() },
		);
		return decodeProgrammerValuesSnapshot(
			await this.responseValue(response),
			scope.userId,
		);
	}

	async applyAction(
		scope: ProgrammerValuesScope,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome> {
		validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchImplementation(
			`${this.valuesPath(scope)}/actions`,
			{
				method: "POST",
				headers,
				body: JSON.stringify(encodeProgrammerValuesActionRequest(request)),
			},
		);
		return decodeProgrammerValuesActionOutcome(
			await this.responseValue(response),
			scope.userId,
			request.requestId,
		);
	}

	subscribe(
		scope: ProgrammerValuesScope,
		afterSequence: number | null,
		observer: ProgrammerValuesEventObserver,
	): ProgrammerValuesEventStream {
		validateScope(scope);
		validateSequence(afterSequence, "event cursor");
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		let explicitlyClosed = false;
		socket.addEventListener("open", () =>
			socket.send(JSON.stringify(subscription(scope.userId, afterSequence))),
		);
		socket.addEventListener("message", (event) =>
			this.deliverEvent(event, scope.userId, observer),
		);
		socket.addEventListener("error", () =>
			observer.error(new Error("Programmer values event connection failed")),
		);
		socket.addEventListener("close", () => {
			if (!explicitlyClosed) observer.closed();
		});
		return {
			repair: (cursor) => {
				validateSequence(cursor, "repair cursor");
				if (socket.readyState !== this.WebSocketImplementation.OPEN) return;
				socket.send(
					JSON.stringify({
						type: "repair",
						cursor: { sequence: cursor },
					} satisfies EventClientMessage),
				);
			},
			close: () => {
				explicitlyClosed = true;
				socket.close();
			},
		};
	}

	private deliverEvent(
		event: MessageEvent,
		userId: string,
		observer: ProgrammerValuesEventObserver,
	) {
		let value: unknown;
		try {
			value = JSON.parse(String(event.data));
			observer.message(decodeProgrammerValuesEventMessage(value, userId));
		} catch (reason) {
			const error =
				reason instanceof Error ? reason : new Error(String(reason));
			observer.error(
				new ProgrammerValuesProtocolError(
					`Invalid Programmer values event: ${error.message}`,
					eventSequence(value),
				),
			);
		}
	}

	private valuesPath(scope: ProgrammerValuesScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-values`;
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

	private async responseValue(response: Response): Promise<unknown> {
		const text = await response.text();
		let value: unknown;
		try {
			value = text ? JSON.parse(text) : null;
		} catch {
			throw fallbackError(response, text);
		}
		if (response.ok) return value;
		try {
			const error = decodeProgrammerValuesErrorResponse(value);
			throw new ProgrammerValuesActionError(
				error.error,
				error.kind,
				response.status,
				error.currentRevision,
				error.currentCaptureModeRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof ProgrammerValuesActionError) throw reason;
			throw fallbackError(response, text);
		}
	}
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
				{ capability: "programmer", id: `programming-values:${userId}` },
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
}

function validateScope(scope: ProgrammerValuesScope) {
	programmerValuesUuidAt(scope.showId, "$.scope.showId");
	programmerValuesUuidAt(scope.userId, "$.scope.userId");
}

function validateSequence(value: number | null, label: string) {
	if (value == null) return;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerValuesProtocolError(
			`Programmer values ${label} must be a non-negative safe integer`,
		);
}

function fallbackError(response: Response, text: string) {
	return new ProgrammerValuesActionError(
		text || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number): ProgrammerValuesErrorKind {
	if (status === 400) return "invalid";
	if (status === 401) return "unauthorized";
	if (status === 403) return "forbidden";
	if (status === 404) return "not_found";
	if (status === 409) return "conflict";
	if (status === 423) return "conflict";
	if (status === 503) return "unavailable";
	return "internal";
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
