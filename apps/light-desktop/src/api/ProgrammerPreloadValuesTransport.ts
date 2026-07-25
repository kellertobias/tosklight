import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesScope,
	ProgrammerPreloadValuesSnapshot,
} from "../features/programmerPreloadValues/contracts";
import type {
	ProgrammerPreloadValuesEventObserver,
	ProgrammerPreloadValuesEventStream,
	ProgrammerPreloadValuesEventTransport,
} from "../features/programmerPreloadValues/transport";
import { ProgrammerPreloadValuesProtocolError } from "../features/programmerPreloadValues/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	decodeProgrammerPreloadValuesErrorResponse,
	decodeProgrammerPreloadValuesEventMessage,
	decodeProgrammerPreloadValuesSnapshot,
	encodeProgrammerPreloadValuesActionRequest,
	type ProgrammerPreloadValuesErrorKind,
} from "./programmerPreloadValuesWire";
import { programmerPreloadValuesUuidAt } from "./programmerPreloadValuesWireProjection";

export interface HttpProgrammerPreloadValuesTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedUserId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

export interface ProgrammerPreloadValuesTransport
	extends ProgrammerPreloadValuesEventTransport {
	loadSnapshot(
		scope: ProgrammerPreloadValuesScope,
	): Promise<ProgrammerPreloadValuesSnapshot>;
	applyAction(
		scope: ProgrammerPreloadValuesScope,
		request: ProgrammerPreloadValuesActionRequest,
	): Promise<ProgrammerPreloadValuesActionOutcome>;
}

export class ProgrammerPreloadValuesActionError extends Error {
	constructor(
		message: string,
		readonly kind: ProgrammerPreloadValuesErrorKind,
		readonly status: number,
		readonly currentPreloadRevision: number | null,
		readonly currentCaptureModeRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerPreloadValuesActionError";
	}
}

/** Dormant until an exact-user Preload values view explicitly uses it. */
export class HttpProgrammerPreloadValuesTransport
	implements ProgrammerPreloadValuesTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: HttpProgrammerPreloadValuesTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(
		scope: ProgrammerPreloadValuesScope,
	): Promise<ProgrammerPreloadValuesSnapshot> {
		this.validateScope(scope);
		const response = await this.fetchImplementation(
			`${this.valuesPath(scope)}/snapshot`,
			{ headers: this.headers() },
		);
		return decodeProgrammerPreloadValuesSnapshot(
			await this.responseValue(response),
			scope.userId,
		);
	}

	async applyAction(
		scope: ProgrammerPreloadValuesScope,
		request: ProgrammerPreloadValuesActionRequest,
	): Promise<ProgrammerPreloadValuesActionOutcome> {
		this.validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchImplementation(
			`${this.valuesPath(scope)}/actions`,
			{
				method: "POST",
				headers,
				body: JSON.stringify(
					encodeProgrammerPreloadValuesActionRequest(request),
				),
			},
		);
		return decodeProgrammerPreloadValuesActionOutcome(
			await this.responseValue(response),
			scope.userId,
			request.requestId,
		);
	}

	subscribe(
		scope: ProgrammerPreloadValuesScope,
		afterSequence: number | null,
		observer: ProgrammerPreloadValuesEventObserver,
	): ProgrammerPreloadValuesEventStream {
		this.validateScope(scope);
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
			observer.error(
				new Error("Preload Programmer values event connection failed"),
			),
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
		observer: ProgrammerPreloadValuesEventObserver,
	) {
		let value: unknown;
		try {
			value = JSON.parse(String(event.data));
			observer.message(
				decodeProgrammerPreloadValuesEventMessage(value, userId),
			);
		} catch (reason) {
			const error =
				reason instanceof Error ? reason : new Error(String(reason));
			observer.error(
				new ProgrammerPreloadValuesProtocolError(
					`Invalid Preload Programmer values event: ${error.message}`,
					eventSequence(value),
				),
			);
		}
	}

	private validateScope(scope: ProgrammerPreloadValuesScope) {
		programmerPreloadValuesUuidAt(scope.showId, "$.scope.showId");
		const userId = programmerPreloadValuesUuidAt(
			scope.userId,
			"$.scope.userId",
		);
		const authenticatedUserId = programmerPreloadValuesUuidAt(
			this.options.authenticatedUserId,
			"$.authenticatedUserId",
		);
		if (userId.toLowerCase() !== authenticatedUserId.toLowerCase())
			throw new ProgrammerPreloadValuesProtocolError(
				"Preload Programmer values scope does not match the authenticated user",
			);
	}

	private valuesPath(scope: ProgrammerPreloadValuesScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-preload-values`;
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
			const error = decodeProgrammerPreloadValuesErrorResponse(value);
			throw new ProgrammerPreloadValuesActionError(
				error.error,
				error.kind,
				response.status,
				error.currentPreloadRevision,
				error.currentCaptureModeRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof ProgrammerPreloadValuesActionError) throw reason;
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
				{
					capability: "programmer",
					id: `programming-preload-values:${userId}`,
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
		throw new ProgrammerPreloadValuesProtocolError(
			`Preload Programmer values ${label} must be a non-negative safe integer`,
		);
}

function fallbackError(response: Response, text: string) {
	return new ProgrammerPreloadValuesActionError(
		text || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number): ProgrammerPreloadValuesErrorKind {
	if (status === 400) return "invalid";
	if (status === 401) return "unauthorized";
	if (status === 403) return "forbidden";
	if (status === 404) return "not_found";
	if (status === 409 || status === 423) return "conflict";
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
