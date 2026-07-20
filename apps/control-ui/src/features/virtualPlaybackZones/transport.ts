import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesScope,
	VirtualPlaybackZonesTransport,
} from "./contracts";
import {
	decodeVirtualPlaybackZonesSaveOutcome,
	decodeVirtualPlaybackZonesSnapshot,
	encodeVirtualPlaybackZonesSaveRequest,
	validateVirtualPlaybackZoneSurfaceId,
	validateVirtualPlaybackZonesScope,
} from "./wire";

export interface HttpVirtualPlaybackZonesTransportOptions {
	readonly baseUrl: string;
	readonly sessionToken: string;
	readonly deskBoundaryToken?: string;
	readonly fetch?: typeof globalThis.fetch;
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

	constructor(private readonly options: HttpVirtualPlaybackZonesTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async loadSnapshot(scope: VirtualPlaybackZonesScope, signal?: AbortSignal) {
		validateVirtualPlaybackZonesScope(scope);
		const response = await this.fetchImplementation(
			`${this.scopeUrl(scope)}/virtual-playback-exclusion-zones`,
			{ headers: this.headers(), signal },
		);
		return decodeVirtualPlaybackZonesSnapshot(
			await responseValue(response),
			scope,
		);
	}

	async saveSurface(
		scope: VirtualPlaybackZonesScope,
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
		signal?: AbortSignal,
	) {
		validateVirtualPlaybackZonesScope(scope);
		validateVirtualPlaybackZoneSurfaceId(surfaceId);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const response = await this.fetchImplementation(
			`${this.scopeUrl(scope)}/virtual-playback-exclusion-zones/${encodeURIComponent(surfaceId)}`,
			{
				method: "PUT",
				headers,
				body: JSON.stringify(encodeVirtualPlaybackZonesSaveRequest(zones)),
				signal,
			},
		);
		return decodeVirtualPlaybackZonesSaveOutcome(
			await responseValue(response),
			scope,
			surfaceId,
		);
	}

	private scopeUrl(scope: VirtualPlaybackZonesScope) {
		return `${this.baseUrl}/api/v2/shows/${encodeURIComponent(scope.showId)}/desks/${encodeURIComponent(scope.deskId)}`;
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
