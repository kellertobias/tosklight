import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesEventObserver,
	VirtualPlaybackZonesEventStream,
	VirtualPlaybackZonesScope,
	VirtualPlaybackZonesTransport,
} from "./contracts";
import {
	decodeVirtualPlaybackZonesEvent,
	decodeVirtualPlaybackZonesSaveOutcome,
	decodeVirtualPlaybackZonesSnapshot,
	encodeVirtualPlaybackZonesSaveRequest,
	validateVirtualPlaybackZonesScope,
} from "./wire";

export interface HttpVirtualPlaybackZonesTransportOptions {
	readonly baseUrl: string;
	readonly sessionToken: string;
	readonly deskBoundaryToken?: string;
	readonly fetch?: typeof globalThis.fetch;
	readonly webSocket?: typeof globalThis.WebSocket;
}

export class VirtualPlaybackZonesHttpError extends Error {
	constructor(
		message: string,
		readonly status: number,
	) {
		super(message);
		this.name = "VirtualPlaybackZonesHttpError";
	}
}

/** Authenticated adapter; construction performs no network work. */
export class HttpVirtualPlaybackZonesTransport
	implements VirtualPlaybackZonesTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(
		private readonly options: HttpVirtualPlaybackZonesTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async loadSnapshot(scope: VirtualPlaybackZonesScope, signal?: AbortSignal) {
		validateVirtualPlaybackZonesScope(scope);
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/virtual-playback-exclusion-zones`,
			{ headers: this.headers(scope), signal },
		);
		return decodeVirtualPlaybackZonesSnapshot(
			await responseValue(response),
			scope,
		);
	}

	async save(
		scope: VirtualPlaybackZonesScope,
		expectedRevision: number,
		zones: readonly VirtualPlaybackZone[],
		requestId: string,
		signal?: AbortSignal,
	) {
		validateVirtualPlaybackZonesScope(scope);
		const headers = this.headers(scope);
		headers.set("content-type", "application/json");
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/virtual-playback-exclusion-zones/update`,
			{
				method: "POST",
				headers,
				body: JSON.stringify(
					encodeVirtualPlaybackZonesSaveRequest(
						requestId,
						expectedRevision,
						zones,
					),
				),
				signal,
			},
		);
		return decodeVirtualPlaybackZonesSaveOutcome(
			await responseValue(response),
			scope,
			requestId,
		);
	}

	subscribe(
		scope: VirtualPlaybackZonesScope,
		observer: VirtualPlaybackZonesEventObserver,
	): VirtualPlaybackZonesEventStream {
		validateVirtualPlaybackZonesScope(scope);
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
			socket.send(
				JSON.stringify({
					type: "subscribe",
					filter: {
						capabilities: ["show"],
						classes: ["projection"],
						objects: [
							{
								capability: "show",
								id: `virtual-playback-exclusion-zones:${scope.showId}`,
							},
						],
					},
					capacity: 32,
					rate_limits: [],
				}),
			);
		});
		socket.addEventListener("message", (event) => {
			try {
				const decoded = decodeVirtualPlaybackZonesEvent(
					JSON.parse(String(event.data)),
				);
				if (decoded === "gap") observer.gap();
				else if (decoded === "error")
					observer.error(new Error("Virtual Playback zone event failed"));
				else if (decoded !== "ready") observer.changed(decoded);
			} catch (reason) {
				observer.error(asError(reason));
			}
		});
		socket.addEventListener("error", () => {
			observer.error(
				new Error("Virtual Playback zone event connection failed"),
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

	private headers(scope: VirtualPlaybackZonesScope) {
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
	return new VirtualPlaybackZonesHttpError(
		body || `${response.status} ${response.statusText}`,
		response.status,
	);
}
