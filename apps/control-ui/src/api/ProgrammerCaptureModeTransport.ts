import type {
	ProgrammerCaptureModeScope,
	ProgrammerCaptureModeSnapshot,
} from "../features/programmerCaptureMode/contracts";
import type {
	ProgrammerCaptureModeEventObserver,
	ProgrammerCaptureModeEventStream,
	ProgrammerCaptureModeEventTransport,
} from "../features/programmerCaptureMode/transport";
import { ProgrammerCaptureModeProtocolError } from "../features/programmerCaptureMode/transport";
import type {
	EventClientMessage,
	ProgrammingValuesErrorKind,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammerCaptureModeEventMessage,
	decodeProgrammerCaptureModeSnapshot,
} from "./programmerCaptureModeWire";
import { programmingUuidAt } from "./programmingWireProjection";

const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly ProgrammingValuesErrorKind[];

export interface HttpProgrammerCaptureModeTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

export interface ProgrammerCaptureModeTransport
	extends ProgrammerCaptureModeEventTransport {
	loadSnapshot(
		scope: ProgrammerCaptureModeScope,
	): Promise<ProgrammerCaptureModeSnapshot>;
}

export class ProgrammerCaptureModeHttpError extends Error {
	constructor(
		message: string,
		readonly kind: ProgrammingValuesErrorKind,
		readonly status: number,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerCaptureModeHttpError";
	}
}

/** Dormant until a caller explicitly loads or subscribes a capture-mode view. */
export class HttpProgrammerCaptureModeTransport
	implements ProgrammerCaptureModeTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: HttpProgrammerCaptureModeTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(
		scope: ProgrammerCaptureModeScope,
	): Promise<ProgrammerCaptureModeSnapshot> {
		validateScope(scope);
		const response = await this.fetchImplementation(this.snapshotPath(scope), {
			headers: this.headers(),
		});
		return decodeProgrammerCaptureModeSnapshot(
			await responseValue(response),
			scope.userId,
		);
	}

	subscribe(
		scope: ProgrammerCaptureModeScope,
		afterSequence: number | null,
		observer: ProgrammerCaptureModeEventObserver,
	): ProgrammerCaptureModeEventStream {
		validateScope(scope);
		validateSequence(afterSequence, "event cursor");
		const socket = this.openEventSocket();
		const lifecycle = { explicitlyClosed: false };
		bindCaptureModeSocket(
			socket,
			scope.userId,
			afterSequence,
			observer,
			lifecycle,
		);
		return eventStream(socket, this.WebSocketImplementation.OPEN, lifecycle);
	}

	private openEventSocket() {
		return new this.WebSocketImplementation(this.eventUrl(), this.protocols());
	}

	private snapshotPath(scope: ProgrammerCaptureModeScope) {
		return `${this.baseUrl}/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-capture-mode/snapshot`;
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

function bindCaptureModeSocket(
	socket: WebSocket,
	userId: string,
	afterSequence: number | null,
	observer: ProgrammerCaptureModeEventObserver,
	lifecycle: SocketLifecycle,
) {
	socket.addEventListener("open", () =>
		socket.send(JSON.stringify(captureModeSubscription(userId, afterSequence))),
	);
	socket.addEventListener("message", (event) =>
		deliverEvent(event, userId, observer),
	);
	socket.addEventListener("error", () =>
		observer.error(
			new Error("Programmer capture mode event connection failed"),
		),
	);
	socket.addEventListener("close", () => {
		if (!lifecycle.explicitlyClosed) observer.closed();
	});
}

function eventStream(
	socket: WebSocket,
	openState: number,
	lifecycle: SocketLifecycle,
): ProgrammerCaptureModeEventStream {
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
	observer: ProgrammerCaptureModeEventObserver,
) {
	let value: unknown;
	try {
		value = JSON.parse(String(event.data));
		observer.message(decodeProgrammerCaptureModeEventMessage(value, userId));
	} catch (reason) {
		const error = reason instanceof Error ? reason : new Error(String(reason));
		observer.error(
			new ProgrammerCaptureModeProtocolError(
				`Invalid Programmer capture mode event: ${error.message}`,
				eventSequence(value),
			),
		);
	}
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

function captureModeSubscription(
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
					id: `programming-capture-mode:${userId}`,
				},
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
}

function validateScope(scope: ProgrammerCaptureModeScope) {
	programmingUuidAt(scope.showId, "$.scope.showId");
	programmingUuidAt(scope.userId, "$.scope.userId");
}

function validateSequence(value: number | null, label: string) {
	if (value == null) return;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerCaptureModeProtocolError(
			`Programmer capture mode ${label} must be a non-negative safe integer`,
		);
}

async function responseValue(response: Response): Promise<unknown> {
	const text = await response.text();
	let value: unknown;
	try {
		value = text ? JSON.parse(text) : null;
	} catch {
		throw fallbackError(response, text);
	}
	if (response.ok) return value;
	throw decodedHttpError(response, value, text);
}

function decodedHttpError(
	response: Response,
	value: unknown,
	fallbackText: string,
) {
	try {
		const body = recordAt(value, "$");
		return new ProgrammerCaptureModeHttpError(
			stringAt(body.error, "$.error"),
			enumAt(body.kind, "$.kind", ERROR_KINDS),
			response.status,
			booleanAt(body.retryable, "$.retryable"),
		);
	} catch {
		return fallbackError(response, fallbackText);
	}
}

function fallbackError(response: Response, text: string) {
	return new ProgrammerCaptureModeHttpError(
		text || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		response.status >= 500,
	);
}

function kindForStatus(status: number): ProgrammingValuesErrorKind {
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
