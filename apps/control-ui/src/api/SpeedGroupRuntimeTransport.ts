import type {
	SpeedGroupActionRequest,
	SpeedGroupRuntimeScope,
} from "../features/speedGroupRuntime/contracts";
import type {
	SpeedGroupEventObserver,
	SpeedGroupEventStream,
	SpeedGroupRuntimeTransport,
} from "../features/speedGroupRuntime/transport";
import {
	SpeedGroupProtocolError,
	SpeedGroupTransportError,
} from "../features/speedGroupRuntime/transport";
import type { EventClientMessage } from "./generated/light-wire";
import { programmingUuidAt } from "./programmingWireProjection";
import {
	decodeSpeedGroupActionOutcome,
	decodeSpeedGroupErrorResponse,
	decodeSpeedGroupEventMessage,
	decodeSpeedGroupSnapshot,
	encodeSpeedGroupActionRequest,
} from "./speedGroupRuntimeWire";

export interface HttpSpeedGroupRuntimeTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedDeskId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

/** Dormant desk-authenticated adapter for installation-global manual speeds. */
export class HttpSpeedGroupRuntimeTransport
	implements SpeedGroupRuntimeTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: HttpSpeedGroupRuntimeTransportOptions) {
		programmingUuidAt(options.authenticatedDeskId, "$.authenticatedDeskId");
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(scope: SpeedGroupRuntimeScope) {
		this.validateScope(scope);
		const response = await this.fetchRequest(this.speedGroupPath(scope), {
			headers: this.headers(),
		});
		return decodeSnapshotResponse(await responseValue(response));
	}

	async applyAction(
		scope: SpeedGroupRuntimeScope,
		request: SpeedGroupActionRequest,
	) {
		this.validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchRequest(this.speedGroupPath(scope), {
			method: "POST",
			headers,
			body: JSON.stringify(encodeSpeedGroupActionRequest(request)),
		});
		return decodeActionResponse(await responseValue(response), request);
	}

	subscribe(
		scope: SpeedGroupRuntimeScope,
		afterSequence: number | null,
		observer: SpeedGroupEventObserver,
	): SpeedGroupEventStream {
		this.validateScope(scope);
		validateSequence(afterSequence, "event cursor");
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		const lifecycle = { explicitlyClosed: false };
		bindSocket(socket, afterSequence, observer, lifecycle);
		return eventStream(socket, this.WebSocketImplementation.OPEN, lifecycle);
	}

	private async fetchRequest(input: string, init: RequestInit) {
		try {
			const response = await this.fetchImplementation(input, init);
			if (response.ok) return response;
			throw await httpError(response);
		} catch (reason) {
			if (reason instanceof SpeedGroupTransportError) throw reason;
			throw unavailableError(reason);
		}
	}

	private validateScope(scope: SpeedGroupRuntimeScope) {
		const deskId = programmingUuidAt(scope.deskId, "$.scope.deskId");
		if (!sameId(deskId, this.options.authenticatedDeskId))
			throw new SpeedGroupProtocolError(
				"Speed Group scope does not match the authenticated desk",
			);
	}

	private speedGroupPath(scope: SpeedGroupRuntimeScope) {
		return `${this.baseUrl}/api/v2/desks/${encodeURIComponent(scope.deskId)}/speed-groups`;
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
	observer: SpeedGroupEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(speedGroupSubscription(afterSequence))),
	);
	socket.addEventListener("message", (event) => deliverEvent(event, observer));
	socket.addEventListener("error", () =>
		observer.error(new Error("Speed Group event connection failed")),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): SpeedGroupEventStream {
	return {
		repair: (cursor) => repairStream(socket, cursor, openState),
		close: () => {
			lifecycle.explicitlyClosed = true;
			socket.close();
		},
	};
}

function deliverEvent(event: MessageEvent, observer: SpeedGroupEventObserver) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(decodeSpeedGroupEventMessage(value));
	} catch (reason) {
		observer.error(
			new SpeedGroupProtocolError(
				`Invalid Speed Group event: ${asError(reason).message}`,
				eventSequence(value),
			),
		);
	}
}

function speedGroupSubscription(
	afterSequence: number | null,
): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: ["playback"],
			classes: ["projection"],
			objects: [{ capability: "playback", id: "speed-groups:manual" }],
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
	} catch (reason) {
		throw new SpeedGroupProtocolError(
			`Invalid Speed Group HTTP response: ${asError(reason).message}`,
		);
	}
}

function decodeSnapshotResponse(value: unknown) {
	try {
		return decodeSpeedGroupSnapshot(value);
	} catch (reason) {
		throw protocolResponseError("snapshot", reason);
	}
}

function decodeActionResponse(
	value: unknown,
	request: SpeedGroupActionRequest,
) {
	try {
		return decodeSpeedGroupActionOutcome(value, request);
	} catch (reason) {
		throw protocolResponseError("action outcome", reason);
	}
}

function protocolResponseError(subject: string, reason: unknown) {
	return reason instanceof SpeedGroupProtocolError
		? reason
		: new SpeedGroupProtocolError(
				`Invalid Speed Group ${subject}: ${asError(reason).message}`,
			);
}

async function httpError(response: Response) {
	const text = await response.text();
	try {
		const value: unknown = text ? JSON.parse(text) : null;
		const error = decodeSpeedGroupErrorResponse(value);
		return new SpeedGroupTransportError(
			error.error,
			error.kind,
			response.status,
			error.currentRevision,
			error.retryable,
		);
	} catch (reason) {
		if (reason instanceof SpeedGroupTransportError) return reason;
		return fallbackError(response, text);
	}
}

function unavailableError(reason: unknown) {
	return new SpeedGroupTransportError(
		asError(reason).message,
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, text: string) {
	return new SpeedGroupTransportError(
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
		throw new SpeedGroupProtocolError(
			`Speed Group ${label} must be a non-negative safe integer`,
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

function sameId(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
