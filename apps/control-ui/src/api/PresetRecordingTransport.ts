import type {
	PresetRecordingRequest,
	PresetRecordingTransport,
} from "../features/presetRecording/contracts";
import {
	decodePresetRecordErrorResponse,
	decodePresetRecordingOutcome,
	encodePresetRecordingRequest,
	type PresetRecordErrorKind,
} from "./presetRecordingWire";

export interface HttpPresetRecordingTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export class PresetRecordingActionError extends Error {
	constructor(
		message: string,
		readonly kind: PresetRecordErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "PresetRecordingActionError";
	}
}

/** Action-only adapter: construction performs no network work. */
export class HttpPresetRecordingTransport
	implements PresetRecordingTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpPresetRecordingTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async record(showId: string, request: PresetRecordingRequest) {
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const url = `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/presets/record`;
		let response: Response;
		try {
			response = await this.fetchImplementation(url, {
				method: "POST",
				headers,
				body: JSON.stringify(encodePresetRecordingRequest(request)),
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		return decodePresetRecordingOutcome(
			await this.responseValue(response),
			request,
		);
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
		try {
			const error = decodePresetRecordErrorResponse(value);
			throw new PresetRecordingActionError(
				error.error,
				error.kind,
				response.status,
				error.currentRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof PresetRecordingActionError) throw reason;
			throw fallbackError(response, text);
		}
	}
}

function unavailableError(reason: unknown) {
	return new PresetRecordingActionError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new PresetRecordingActionError(
		body || `${response.status} ${response.statusText}`,
		"internal",
		response.status,
		null,
		response.status >= 500,
	);
}
