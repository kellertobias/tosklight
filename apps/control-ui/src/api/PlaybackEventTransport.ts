import { identityKey } from "../features/playbackRuntime/contracts";
import {
	type PlaybackEventObserver,
	type PlaybackEventScope,
	type PlaybackEventStream,
	type PlaybackEventTransport,
	PlaybackProtocolError,
} from "../features/playbackRuntime/transport";
import type { EventClientMessage } from "./generated/light-wire";
import { decodePlaybackEventMessage } from "./playbackWire";

export interface PlaybackEventTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	webSocket?: typeof globalThis.WebSocket;
}

export class WebSocketPlaybackEventTransport implements PlaybackEventTransport {
	private readonly baseUrl: string;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: PlaybackEventTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	subscribe(
		deskId: string,
		scope: PlaybackEventScope,
		afterSequence: number | null,
		observer: PlaybackEventObserver,
	): PlaybackEventStream {
		const socket = new this.WebSocketImplementation(
			this.eventUrl(),
			this.protocols(),
		);
		let explicitlyClosed = false;
		socket.addEventListener("open", () =>
			socket.send(JSON.stringify(subscription(deskId, scope, afterSequence))),
		);
		socket.addEventListener("message", (event) => {
			let value: unknown;
			try {
				value = JSON.parse(String(event.data));
				const message = decodePlaybackEventMessage(value);
				if (message) observer.message(message);
			} catch (reason) {
				const error =
					reason instanceof Error ? reason : new Error(String(reason));
				observer.error(
					new PlaybackProtocolError(
						`Invalid Playback event: ${error.message}`,
						eventSequence(value),
					),
				);
			}
		});
		socket.addEventListener("error", () =>
			observer.error(new Error("Playback event connection failed")),
		);
		socket.addEventListener("close", () => {
			if (!explicitlyClosed) observer.closed();
		});
		return {
			repair: (cursor) => {
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
	scope: PlaybackEventScope,
	afterSequence: number | null,
): EventClientMessage {
	return {
		type: "subscribe",
		filter: {
			capabilities: scope.desk ? ["playback", "desk"] : ["playback"],
			classes: ["transition", "projection"],
			objects: [
				...scope.identities.map((identity) => ({
					capability: "playback" as const,
					id: identityKey(identity),
				})),
				...(scope.desk
					? [{ capability: "desk" as const, id: `playback-view:${deskId}` }]
					: []),
			],
		},
		after_sequence: afterSequence,
		capacity: 128,
		rate_limits: [],
	};
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
