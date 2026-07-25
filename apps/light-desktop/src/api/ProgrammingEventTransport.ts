import type {
	ProgrammingEventObserver,
	ProgrammingEventScope,
	ProgrammingEventStream,
	ProgrammingEventTransport,
} from "../features/programmingInteraction/transport";
import { ProgrammingProtocolError } from "../features/programmingInteraction/transport";
import type { EventClientMessage } from "./generated/light-wire";
import { decodeProgrammingEventMessage } from "./programmingWire";

export interface ProgrammingEventTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	webSocket?: typeof globalThis.WebSocket;
}

/** Lossless, view-scoped v2 adapter for one desk's Programming interaction. */
export class WebSocketProgrammingEventTransport
	implements ProgrammingEventTransport
{
	private readonly baseUrl: string;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: ProgrammingEventTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	subscribe(
		deskId: string,
		scope: ProgrammingEventScope,
		afterSequence: number | null,
		observer: ProgrammingEventObserver,
	): ProgrammingEventStream {
		const subscribedScope = validateSubscription(deskId, scope, afterSequence);
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		let explicitlyClosed = false;
		socket.addEventListener("open", () =>
			socket.send(
				JSON.stringify(subscription(deskId, subscribedScope, afterSequence)),
			),
		);
		socket.addEventListener("message", (event) => {
			let value: unknown;
			try {
				value = JSON.parse(String(event.data));
				const message = decodeProgrammingEventMessage(
					value,
					deskId,
					subscribedScope,
				);
				if (message) observer.message(message);
			} catch (reason) {
				const error = reason instanceof Error ? reason : new Error(String(reason));
				observer.error(
					new ProgrammingProtocolError(
						`Invalid Programming event: ${error.message}`,
						eventSequence(value),
					),
				);
			}
		});
		socket.addEventListener("error", () =>
			observer.error(new Error("Programming event connection failed")),
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
}

function subscription(
	deskId: string,
	scope: ProgrammingEventScope,
	afterSequence: number | null,
): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: ["desk"],
			classes: ["projection"],
			objects: [
				...(scope.commandLine
					? [
							{
								capability: "desk" as const,
								id: `programming-command-line:${deskId}`,
							},
						]
					: []),
				...(scope.selection
					? [
							{
								capability: "desk" as const,
								id: `programming-selection:${deskId}`,
							},
						]
					: []),
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
}

function validateSubscription(
	deskId: string,
	scope: ProgrammingEventScope,
	afterSequence: number | null,
): ProgrammingEventScope {
	if (!isUuid(deskId))
		throw new ProgrammingProtocolError(
			"Programming subscription requires a UUID desk ID",
		);
	if (
		typeof scope.commandLine !== "boolean" ||
		typeof scope.selection !== "boolean" ||
		(!scope.commandLine && !scope.selection)
	)
		throw new ProgrammingProtocolError(
			"Programming subscription requires at least one valid view capability",
		);
	if (afterSequence != null) validateSequence(afterSequence, "event cursor");
	return {
		commandLine: scope.commandLine,
		selection: scope.selection,
	};
}

function validateSequence(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammingProtocolError(
			`Programming ${label} must be a non-negative safe integer`,
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

function isUuid(value: string) {
	return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
		value,
	);
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
