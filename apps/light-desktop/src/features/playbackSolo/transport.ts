import type {
	PlaybackSoloRegion,
	PlaybackSoloEventObserver,
	PlaybackSoloEventStream,
	PlaybackSoloScope,
	PlaybackSoloTransport,
} from "./contracts";
import {
	decodePlaybackSoloEvent,
	decodePlaybackSoloSaveOutcome,
	decodePlaybackSoloSnapshot,
	encodePlaybackSoloSaveRequest,
	validatePlaybackSoloScope,
} from "./wire";

export interface HttpPlaybackSoloTransportOptions {
	readonly baseUrl: string;
	readonly sessionToken: string;
	readonly deskBoundaryToken?: string;
	readonly fetch?: typeof globalThis.fetch;
	readonly webSocket?: typeof globalThis.WebSocket;
}

export class PlaybackSoloHttpError extends Error {
	constructor(
		message: string,
		readonly status: number,
	) {
		super(message);
		this.name = "PlaybackSoloHttpError";
	}
}

/** Authenticated adapter; construction performs no network work. */
export class HttpPlaybackSoloTransport
	implements PlaybackSoloTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: HttpPlaybackSoloTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(scope: PlaybackSoloScope, signal?: AbortSignal) {
		validatePlaybackSoloScope(scope);
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/playback-solo-regions`,
			{ headers: this.headers(scope), signal },
		);
		return decodePlaybackSoloSnapshot(
			await responseValue(response),
			scope,
		);
	}

	async save(
		scope: PlaybackSoloScope,
		expectedRevision: number,
		regions: readonly PlaybackSoloRegion[],
		requestId: string,
		signal?: AbortSignal,
	) {
		validatePlaybackSoloScope(scope);
		const headers = this.headers(scope);
		headers.set("content-type", "application/json");
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/playback-solo-regions/update`,
			{
				method: "POST",
				headers,
				body: JSON.stringify(
					encodePlaybackSoloSaveRequest(
						requestId,
						expectedRevision,
						regions,
					),
				),
				signal,
			},
		);
		return decodePlaybackSoloSaveOutcome(
			await responseValue(response),
			scope,
			requestId,
		);
	}

	subscribe(
		scope: PlaybackSoloScope,
		observer: PlaybackSoloEventObserver,
	): PlaybackSoloEventStream {
		validatePlaybackSoloScope(scope);
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
			if (explicitlyClosed) return;
			socket.send(
				JSON.stringify({
					type: "subscribe",
					filter: {
						capabilities: ["show"],
						classes: ["projection"],
						objects: [
							{
								capability: "show",
								id: `playback-solo-regions:${scope.showId}`,
							},
						],
					},
					capacity: 32,
					rate_limits: [],
				}),
			);
		});
		socket.addEventListener("message", (event) => {
			if (explicitlyClosed) return;
			try {
				const decoded = decodePlaybackSoloEvent(
					JSON.parse(String(event.data)),
				);
				if (decoded === "gap") observer.gap();
				else if (decoded === "error")
					observer.error(new Error("Solo Region event failed"));
				else if (decoded === "ready") observer.ready?.();
				else observer.changed(decoded);
			} catch (reason) {
				observer.error(asError(reason));
			}
		});
		socket.addEventListener("error", () => {
			if (explicitlyClosed) return;
			observer.error(
				new Error("Solo Region event connection failed"),
			);
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

	private headers(scope: PlaybackSoloScope) {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
			"x-tosk-show": scope.showId,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
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

async function responseValue(response: Response): Promise<unknown> {
	const text = await response.text();
	let value: unknown;
	try {
		value = text ? JSON.parse(text) : null;
	} catch {
		throw httpError(response, text);
	}
	if (response.ok) return value;
	throw httpError(response, errorMessage(value) ?? text);
}

function errorMessage(value: unknown) {
	if (!value || typeof value !== "object" || Array.isArray(value)) return null;
	const error = (value as Record<string, unknown>).error;
	return typeof error === "string" && error ? error : null;
}

function httpError(response: Response, body: string) {
	return new PlaybackSoloHttpError(
		body || `${response.status} ${response.statusText}`,
		response.status,
	);
}
