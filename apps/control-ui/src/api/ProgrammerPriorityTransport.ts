import type {
	ProgrammerPriorityActionRequest,
	ProgrammerPriorityScope,
} from "../features/programmerPriority/contracts";
import type {
	ProgrammerPriorityEventObserver,
	ProgrammerPriorityEventStream,
	ProgrammerPriorityTransport,
} from "../features/programmerPriority/transport";
import {
	ProgrammerPriorityProtocolError,
	ProgrammerPriorityTransportError,
} from "../features/programmerPriority/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeProgrammerPriorityActionOutcome,
	decodeProgrammerPriorityErrorResponse,
	decodeProgrammerPriorityEventMessage,
	decodeProgrammerPrioritySnapshot,
	encodeProgrammerPriorityActionRequest,
} from "./programmerPriorityWire";
import { programmingUuidAt } from "./programmingWireProjection";

export interface HttpProgrammerPriorityTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedUserId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

/** Dormant exact-user HTTP and WebSocket adapter for Programmer priority. */
export class HttpProgrammerPriorityTransport
	implements ProgrammerPriorityTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: HttpProgrammerPriorityTransportOptions,
	) {
		programmingUuidAt(options.authenticatedUserId, "$.authenticatedUserId");
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(scope: ProgrammerPriorityScope) {
		this.validateScope(scope);
		const response = await this.fetchRequest(this.snapshotPath(scope), {
			headers: this.headers(),
		});
		return decodeProgrammerPrioritySnapshot(
			await responseValue(response),
			scope.userId,
		);
	}

	async applyAction(
		scope: ProgrammerPriorityScope,
		request: ProgrammerPriorityActionRequest,
	) {
		this.validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchRequest(this.actionPath(scope), {
			method: "POST",
			headers,
			body: JSON.stringify(encodeProgrammerPriorityActionRequest(request)),
		});
		return decodeProgrammerPriorityActionOutcome(
			await responseValue(response),
			scope.userId,
			request,
		);
	}

	subscribe(
		scope: ProgrammerPriorityScope,
		afterSequence: number | null,
		observer: ProgrammerPriorityEventObserver,
	): ProgrammerPriorityEventStream {
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

	private async fetchRequest(input: string, init: RequestInit) {
		try {
			const response = await this.fetchImplementation(input, init);
			if (response.ok) return response;
			throw await httpError(response);
		} catch (reason) {
			if (reason instanceof ProgrammerPriorityTransportError) throw reason;
			throw unavailableError(reason);
		}
	}

	private validateScope(scope: ProgrammerPriorityScope) {
		const userId = programmingUuidAt(scope.userId, "$.scope.userId");
		if (userId.toLowerCase() !== this.options.authenticatedUserId.toLowerCase())
			throw new ProgrammerPriorityProtocolError(
				"Programmer priority scope does not match the authenticated user",
			);
	}

	private snapshotPath(scope: ProgrammerPriorityScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-priority/snapshot`;
	}

	private actionPath(scope: ProgrammerPriorityScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-priority/actions`;
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
	observer: ProgrammerPriorityEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(prioritySubscription(userId, afterSequence))),
	);
	socket.addEventListener("message", (event) =>
		deliverEvent(event, userId, observer),
	);
	socket.addEventListener("error", () =>
		observer.error(new Error("Programmer priority event connection failed")),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): ProgrammerPriorityEventStream {
	return {
		repair: (cursor) => repairStream(socket, cursor, openState),
		close: () => {
			lifecycle.explicitlyClosed = true;
			socket.close();
		},
	};
}

function deliverEvent(
	event: MessageEvent,
	userId: string,
	observer: ProgrammerPriorityEventObserver,
) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(decodeProgrammerPriorityEventMessage(value, userId));
	} catch (reason) {
		observer.error(
			new ProgrammerPriorityProtocolError(
				`Invalid Programmer priority event: ${asError(reason).message}`,
				eventSequence(value),
			),
		);
	}
}

function prioritySubscription(
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
					id: `programming-priority:${userId}`,
				},
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
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

async function responseValue(response: Response) {
	const text = await response.text();
	try {
		return text ? (JSON.parse(text) as unknown) : null;
	} catch {
		throw fallbackError(response, text);
	}
}

async function httpError(response: Response) {
	const text = await response.text();
	let value: unknown;
	try {
		value = text ? JSON.parse(text) : null;
		const error = decodeProgrammerPriorityErrorResponse(value);
		return new ProgrammerPriorityTransportError(
			error.error,
			error.kind,
			response.status,
			error.currentRevision,
			error.retryable,
		);
	} catch (reason) {
		if (reason instanceof ProgrammerPriorityTransportError) return reason;
		return fallbackError(response, text);
	}
}

function unavailableError(reason: unknown) {
	return new ProgrammerPriorityTransportError(
		asError(reason).message,
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, text: string) {
	return new ProgrammerPriorityTransportError(
		text || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number) {
	if (status === 401) return "unauthorized" as const;
	if (status === 403) return "forbidden" as const;
	if (status === 404) return "not_found" as const;
	if (status === 409 || status === 423) return "conflict" as const;
	if (status === 503) return "unavailable" as const;
	return status >= 500 ? ("internal" as const) : ("invalid" as const);
}

function validateSequence(value: number | null, label: string) {
	if (value == null) return;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerPriorityProtocolError(
			`Programmer priority ${label} must be a non-negative safe integer`,
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

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
