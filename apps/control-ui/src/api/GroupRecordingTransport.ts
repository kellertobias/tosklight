import type {
	GroupRecordingRequest,
	GroupRecordingTransport,
} from "../features/groupRecording/contracts";
import {
	decodeGroupRecordErrorResponse,
	decodeGroupRecordingOutcome,
	encodeGroupRecordingRequest,
	type GroupRecordErrorKind,
} from "./groupRecordingWire";
import { WireValidationError } from "./wireValidation";

export interface HttpGroupRecordingTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export class GroupRecordingActionError extends Error {
	constructor(
		message: string,
		readonly kind: GroupRecordErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "GroupRecordingActionError";
	}
}

/** Action-only adapter: construction performs no network work. */
export class HttpGroupRecordingTransport implements GroupRecordingTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpGroupRecordingTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async record(showId: string, request: GroupRecordingRequest) {
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const url = `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/groups/record`;
		let response: Response;
		try {
			response = await this.fetchImplementation(url, {
				method: "POST",
				headers,
				body: JSON.stringify(encodeGroupRecordingRequest(request)),
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		const outcome = decodeGroupRecordingOutcome(
			await this.responseValue(response),
			request,
		);
		verifyRevisionEtag(response, outcome.group.revision);
		return outcome;
	}

	private headers() {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
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
		let error: ReturnType<typeof decodeGroupRecordErrorResponse>;
		try {
			error = decodeGroupRecordErrorResponse(value);
		} catch {
			throw fallbackError(response, text);
		}
		if (error.currentRevision != null)
			verifyRevisionEtag(response, error.currentRevision);
		throw new GroupRecordingActionError(
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
	return new GroupRecordingActionError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new GroupRecordingActionError(
		body || `${response.status} ${response.statusText}`,
		"internal",
		response.status,
		null,
		response.status >= 500,
	);
}
