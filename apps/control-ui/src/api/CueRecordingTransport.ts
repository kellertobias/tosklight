import type {
	CueRecordingRequest,
	CueRecordingTransport,
} from "../features/cueRecording/contracts";
import {
	decodeCueRecordErrorResponse,
	decodeCueRecordingOutcome,
	encodeCueRecordingRequest,
	type CueRecordErrorKind,
} from "./cueRecordingWire";
import { WireValidationError } from "./wireValidation";

export interface HttpCueRecordingTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export class CueRecordingActionError extends Error {
	constructor(
		message: string,
		readonly kind: CueRecordErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "CueRecordingActionError";
	}
}

/** Action-only adapter: construction performs no network work. */
export class HttpCueRecordingTransport implements CueRecordingTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpCueRecordingTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation = options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async record(
		showId: string,
		expectedShowRevision: number,
		request: CueRecordingRequest,
	) {
		const headers = this.headers(expectedShowRevision);
		const url = `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/cues/record`;
		let response: Response;
		try {
			response = await this.fetchImplementation(url, {
				method: "POST",
				headers,
				body: JSON.stringify(encodeCueRecordingRequest(request)),
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		const value = await this.responseValue(response);
		const outcome = decodeCueRecordingOutcome(
			value,
			request,
			showId,
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
		let error: ReturnType<typeof decodeCueRecordErrorResponse>;
		try {
			error = decodeCueRecordErrorResponse(value);
		} catch {
			throw fallbackError(response, text);
		}
		if (error.currentRevision != null)
			verifyRevisionEtag(response, error.currentRevision);
		throw new CueRecordingActionError(
			error.error,
			error.kind,
			response.status,
			error.currentRevision,
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
	return new CueRecordingActionError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new CueRecordingActionError(
		body || `${response.status} ${response.statusText}`,
		"internal",
		response.status,
		null,
		response.status >= 500,
	);
}
