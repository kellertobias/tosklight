import type {
	PlaybackTopologyRequest,
	PlaybackTopologyTransport,
} from "../features/playbackTopology/contracts";
import {
	decodePlaybackTopologyErrorResponse,
	decodePlaybackTopologyOutcome,
	encodePlaybackTopologyRequest,
	type PlaybackTopologyErrorKind,
} from "./playbackTopologyWire";
import { WireValidationError } from "./wireValidation";

export interface HttpPlaybackTopologyTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export class PlaybackTopologyActionError extends Error {
	constructor(
		message: string,
		readonly kind: PlaybackTopologyErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly currentRelatedRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "PlaybackTopologyActionError";
	}
}

/** Strict action-only HTTP adapter; construction performs no network work. */
export class HttpPlaybackTopologyTransport
	implements PlaybackTopologyTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpPlaybackTopologyTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation = options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async apply(
		showId: string,
		expectedShowRevision: number,
		request: PlaybackTopologyRequest,
	) {
		const url = `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/playback-topology/actions`;
		let response: Response;
		try {
			response = await this.fetchImplementation(url, {
				method: "POST",
				headers: this.headers(expectedShowRevision),
				body: JSON.stringify(encodePlaybackTopologyRequest(request)),
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		const value = await this.responseValue(response);
		const outcome = decodePlaybackTopologyOutcome(
			value,
			request,
			expectedShowRevision,
		);
		verifyRevisionEtag(response, outcome.showRevision);
		return outcome;
	}

	private headers(revision: number) {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
			"content-type": "application/json",
			"if-match": `"${revision}"`,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}

	private async responseValue(response: Response): Promise<unknown> {
		const text = await response.text();
		let value: unknown;
		try {
			value = text ? JSON.parse(text) : null;
		} catch {
			throw fallbackError(response, text);
		}
		if (response.ok) return value;
		let error: ReturnType<typeof decodePlaybackTopologyErrorResponse>;
		try {
			error = decodePlaybackTopologyErrorResponse(value);
		} catch {
			throw fallbackError(response, text);
		}
		if (error.currentRevision != null)
			verifyRevisionEtag(response, error.currentRevision);
		throw new PlaybackTopologyActionError(
			error.error,
			error.kind,
			response.status,
			error.currentRevision,
			error.currentRelatedRevision,
			error.retryable,
		);
	}
}

function verifyRevisionEtag(response: Response, revision: number) {
	const expected = `"${revision}"`;
	const actual = response.headers.get("etag");
	if (actual !== expected)
		throw new WireValidationError(
			"$.headers.etag",
			`quoted revision ${expected}`,
			actual,
		);
}

function unavailableError(reason: unknown) {
	return new PlaybackTopologyActionError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new PlaybackTopologyActionError(
		body || `${response.status} ${response.statusText}`,
		"internal",
		response.status,
		null,
		null,
		response.status >= 500,
	);
}
