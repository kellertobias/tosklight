import type {
	OutputRuntimeActionRequest,
	OutputRuntimeScope,
} from "../features/outputRuntime/contracts";
import type {
	OutputRuntimeEventObserver,
	OutputRuntimeEventStream,
	OutputRuntimeTransport,
} from "../features/outputRuntime/transport";
import {
	OutputRuntimeProtocolError,
	OutputRuntimeTransportError,
} from "../features/outputRuntime/transport";
import type { EventClientMessage } from "./generated/light-wire";
import {
	decodeOutputRuntimeActionOutcome,
	decodeOutputRuntimeErrorResponse,
	decodeOutputRuntimeEventMessage,
	decodeOutputRuntimeSnapshot,
	encodeOutputRuntimeActionRequest,
} from "./outputRuntimeWire";
import { programmingUuidAt } from "./programmingWireProjection";

export interface HttpOutputRuntimeTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedDeskId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

/** Dormant desk-authenticated adapter for the installation-global output object. */
export class HttpOutputRuntimeTransport implements OutputRuntimeTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: HttpOutputRuntimeTransportOptions) {
		programmingUuidAt(options.authenticatedDeskId, "$.authenticatedDeskId");
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(scope: OutputRuntimeScope) {
		this.validateScope(scope);
		const response = await this.fetchRequest(this.outputPath(scope), {
			headers: this.headers(),
		});
		return decodeOutputRuntimeSnapshot(
			await responseValue(response),
			scope.showId,
		);
	}

	async applyAction(
		scope: OutputRuntimeScope,
		request: OutputRuntimeActionRequest,
	) {
		this.validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchRequest(this.outputPath(scope), {
			method: "POST",
			headers,
			body: JSON.stringify(encodeOutputRuntimeActionRequest(request)),
		});
		return decodeOutputRuntimeActionOutcome(
			await responseValue(response),
			scope.showId,
			request,
		);
	}

	subscribe(
		scope: OutputRuntimeScope,
		afterSequence: number | null,
		observer: OutputRuntimeEventObserver,
	): OutputRuntimeEventStream {
		this.validateScope(scope);
		validateSequence(afterSequence, "event cursor");
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		const lifecycle = { explicitlyClosed: false };
		bindSocket(socket, scope.showId, afterSequence, observer, lifecycle);
		return eventStream(socket, this.WebSocketImplementation.OPEN, lifecycle);
	}

	private async fetchRequest(input: string, init: RequestInit) {
		try {
			const response = await this.fetchImplementation(input, init);
			if (response.ok) return response;
			throw await httpError(response);
		} catch (reason) {
			if (reason instanceof OutputRuntimeTransportError) throw reason;
			throw unavailableError(reason);
		}
	}

	private validateScope(scope: OutputRuntimeScope) {
		programmingUuidAt(scope.showId, "$.scope.showId");
		const deskId = programmingUuidAt(scope.deskId, "$.scope.deskId");
		if (deskId.toLowerCase() !== this.options.authenticatedDeskId.toLowerCase())
			throw new OutputRuntimeProtocolError(
				"Output runtime scope does not match the authenticated desk",
			);
	}

	private outputPath(scope: OutputRuntimeScope) {
		return `${this.baseUrl}/api/v2/desks/${encodeURIComponent(scope.deskId)}/output-runtime/global-master`;
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
	showId: string,
	afterSequence: number | null,
	observer: OutputRuntimeEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(outputSubscription(afterSequence))),
	);
	socket.addEventListener("message", (event) =>
		deliverEvent(event, showId, observer),
	);
	socket.addEventListener("error", () =>
		observer.error(new Error("Output runtime event connection failed")),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): OutputRuntimeEventStream {
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
	showId: string,
	observer: OutputRuntimeEventObserver,
) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(decodeOutputRuntimeEventMessage(value, showId));
	} catch (reason) {
		observer.error(
			new OutputRuntimeProtocolError(
				`Invalid Output runtime event: ${asError(reason).message}`,
				eventSequence(value),
			),
		);
	}
}

function outputSubscription(afterSequence: number | null): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: ["output"],
			classes: ["projection"],
			objects: [{ capability: "output", id: "runtime:global-master" }],
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
		const error = decodeOutputRuntimeErrorResponse(value);
		return new OutputRuntimeTransportError(
			error.error,
			error.kind,
			response.status,
			error.currentRevision,
			error.retryable,
		);
	} catch (reason) {
		if (reason instanceof OutputRuntimeTransportError) return reason;
		return fallbackError(response, text);
	}
}

function unavailableError(reason: unknown) {
	return new OutputRuntimeTransportError(
		asError(reason).message,
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, text: string) {
	return new OutputRuntimeTransportError(
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
		throw new OutputRuntimeProtocolError(
			`Output runtime ${label} must be a non-negative safe integer`,
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
