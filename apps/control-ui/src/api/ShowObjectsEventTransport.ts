import type { EventClientMessage } from "./generated/light-wire";
import type {
	ShowObjectsEventObserver,
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
					objects: [{ capability: "show", id: `objects:${showId}` }],
				},
				after_sequence: afterSequence,
				capacity: 128,
				rate_limits: [],
			};
			socket.send(JSON.stringify(request));
		});
		socket.addEventListener("message", (event) => {
			try {
				const message = decodeShowObjectsEventMessage(
					JSON.parse(String(event.data)),
				);
				if (message) observer.message(message);
			} catch (reason) {
				observer.error(asError(reason));
			}
		});
		socket.addEventListener("error", () => {
			observer.error(new Error("Show-object event connection failed"));
		});
		socket.addEventListener("close", () => {
			if (!explicitlyClosed) observer.closed();
		});
		return {
			close: () => {
				explicitlyClosed = true;
				socket.close();
			},
		};
	}
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
