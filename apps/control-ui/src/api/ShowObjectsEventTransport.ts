import type { EventClientMessage } from "./generated/light-wire";
import { ShowObjectsProtocolError } from "../features/showObjects/transport";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventStream,
	ShowObjectsEventTransport,
} from "../features/showObjects/transport";
import { decodeShowObjectsEventMessage } from "./showObjectsWire";

export interface ShowObjectsEventTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	webSocket?: typeof globalThis.WebSocket;
}

/** View-scoped v2 event adapter for active-show Group/Preset projections. */
export class WebSocketShowObjectsEventTransport
	implements ShowObjectsEventTransport
{
	private readonly baseUrl: string;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: ShowObjectsEventTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	subscribe(
		showId: string,
		scope: ShowObjectsEventScope,
		afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	): ShowObjectsEventStream {
		const url = new URL("/api/v2/events", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		const protocols = [
			"light.events.v2",
			`light.token.${this.options.sessionToken}`,
		];
		if (this.options.deskBoundaryToken)
			protocols.push(
				`light.desk.b64.${base64Url(this.options.deskBoundaryToken)}`,
			);
		const socket = new this.WebSocketImplementation(url, protocols);
		let explicitlyClosed = false;
		socket.addEventListener("open", () => {
			const request: EventClientMessage = {
				type: "subscribe",
				filter: {
					capabilities: ["show"],
					classes: ["projection"],
					objects: subscriptionObjects(showId, scope),
				},
				after_sequence: afterSequence,
				capacity: 128,
				rate_limits: [],
			};
			socket.send(JSON.stringify(request));
		});
		socket.addEventListener("message", (event) => {
			let value: unknown;
			try {
				value = JSON.parse(String(event.data));
				const message = decodeShowObjectsEventMessage(value);
				if (message) observer.message(message);
			} catch (reason) {
				const error = asError(reason);
				observer.error(
					new ShowObjectsProtocolError(
						`Invalid show-object event: ${error.message}`,
						eventSequence(value),
					),
				);
			}
		});
		socket.addEventListener("error", () => {
			observer.error(new Error("Show-object event connection failed"));
		});
		socket.addEventListener("close", () => {
			if (!explicitlyClosed) observer.closed();
		});
		return {
			repair: (cursor) => {
				if (socket.readyState !== this.WebSocketImplementation.OPEN) return;
				const request: EventClientMessage = {
					type: "repair",
					cursor: { sequence: cursor },
				};
				socket.send(JSON.stringify(request));
			},
			close: () => {
				explicitlyClosed = true;
				socket.close();
			},
		};
	}
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

function subscriptionObjects(showId: string, scope: ShowObjectsEventScope) {
	return [
		...scope.kinds.map((kind) => ({
			capability: "show" as const,
			id: `objects:${showId}:kind:${kind}`,
		})),
		...scope.objects.map(({ kind, objectId }) => ({
			capability: "show" as const,
			id: `objects:${showId}:kind:${kind}:object:${objectId}`,
		})),
	];
}

function base64Url(value: string): string {
	const bytes = new TextEncoder().encode(value);
	let binary = "";
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary)
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replace(/=+$/, "");
}

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}
