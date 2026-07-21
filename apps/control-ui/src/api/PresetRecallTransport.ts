import type {
	PresetRecallRequest,
	PresetRecallScope,
	PresetRecallTransport,
} from "../features/presetRecall/contracts";
import { PresetRecallTransportError } from "../features/presetRecall/contracts";
import {
	decodePresetRecallErrorResponse,
	decodePresetRecallOutcome,
	encodePresetRecallRequest,
	type PresetRecallErrorKind,
} from "./presetRecallWire";
import { programmerValuesUuidAt } from "./programmerValuesWireProjection";

export interface HttpPresetRecallTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

/** Action-only adapter: construction performs no fetch or subscription. */
export class HttpPresetRecallTransport implements PresetRecallTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: HttpPresetRecallTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async recall(scope: PresetRecallScope, request: PresetRecallRequest) {
		validateScope(scope);
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const path = `/api/v2/shows/${encodeURIComponent(scope.showId)}/presets/recall`;
		let response: Response;
		try {
			response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
				method: "POST",
				headers,
				body: JSON.stringify(encodePresetRecallRequest(request)),
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		return decodePresetRecallOutcome(
			await this.responseValue(response),
			scope.userId,
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
			const error = decodePresetRecallErrorResponse(value);
			throw new PresetRecallTransportError(
				error.error,
				error.kind,
				response.status,
				error.currentRevision,
				error.currentRelatedRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof PresetRecallTransportError) throw reason;
			throw fallbackError(response, text);
		}
	}
}

function validateScope(scope: PresetRecallScope) {
	programmerValuesUuidAt(scope.showId, "$.scope.showId");
	programmerValuesUuidAt(scope.userId, "$.scope.userId");
	programmerValuesUuidAt(scope.deskId, "$.scope.deskId");
}

function unavailableError(reason: unknown) {
	return new PresetRecallTransportError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new PresetRecallTransportError(
		body || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number): PresetRecallErrorKind {
	if (status === 401) return "unauthorized";
	if (status === 403) return "forbidden";
	if (status === 404) return "not_found";
	if (status === 409) return "conflict";
	if (status === 503) return "unavailable";
	return status >= 500 ? "internal" : "invalid";
}
