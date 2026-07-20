import {
	type ProgrammingUpdateActionRequest,
	type ProgrammingUpdatePreviewRequest,
	type ProgrammingUpdateTargetsRequest,
	type ProgrammingUpdateTransport,
	ProgrammingUpdateTransportError,
} from "../features/programmingUpdate/contracts";
import type { ProgrammingUpdateErrorKind } from "./generated/light-wire";
import { integerAt } from "./playbackWirePrimitives";
import {
	programmingUpdateActionOutcome,
	programmingUpdatePreviewResponse,
	programmingUpdateSettingsProjection,
	programmingUpdateTargetsResponse,
	wireActionRequest,
	wirePreviewRequest,
	wireSettings,
	wireTargetsRequest,
} from "./programmingUpdateMapping";
import {
	decodeProgrammingUpdateActionOutcome,
	decodeProgrammingUpdateErrorResponse,
	decodeProgrammingUpdatePreviewResponse,
	decodeProgrammingUpdateSettingsProjection,
	decodeProgrammingUpdateTargetsResponse,
	encodeProgrammingUpdateActionRequest,
	encodeProgrammingUpdatePreviewRequest,
	encodeProgrammingUpdateSettings,
	encodeProgrammingUpdateTargetsRequest,
} from "./programmingUpdateWire";
import { scopedUuidAt } from "./programmingUpdateWireShared";
import { WireValidationError } from "./wireValidation";

export interface ProgrammingUpdateTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

export class ProgrammingUpdateHttpError extends ProgrammingUpdateTransportError {
	constructor(
		message: string,
		readonly kind: ProgrammingUpdateErrorKind,
		readonly status: number,
		readonly currentObjectRevision: number | null,
		readonly currentShowRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message, status, currentShowRevision, retryable);
		this.name = "ProgrammingUpdateHttpError";
	}
}

/** Strict Programming Update HTTP adapter; construction performs no I/O. */
export class HttpProgrammingUpdateTransport
	implements ProgrammingUpdateTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(private readonly options: ProgrammingUpdateTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/+$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async preview(showId: string, request: ProgrammingUpdatePreviewRequest) {
		validateScopeId(showId, "show_id");
		const wireRequest = wirePreviewRequest(request);
		const body = encodeProgrammingUpdatePreviewRequest(wireRequest);
		const result = await this.post(this.showPath(showId, "preview"), body);
		const decoded = decodeProgrammingUpdatePreviewResponse(
			result.value,
			showId,
			wireRequest,
		);
		verifyRevisionEtag(result.response, decoded.show_revision);
		return programmingUpdatePreviewResponse(decoded);
	}

	async targets(showId: string, request: ProgrammingUpdateTargetsRequest) {
		validateScopeId(showId, "show_id");
		const wireRequest = wireTargetsRequest(request);
		const body = encodeProgrammingUpdateTargetsRequest(wireRequest);
		const result = await this.post(this.showPath(showId, "targets"), body);
		const decoded = decodeProgrammingUpdateTargetsResponse(
			result.value,
			showId,
			wireRequest,
		);
		verifyRevisionEtag(result.response, decoded.show_revision);
		return programmingUpdateTargetsResponse(decoded);
	}

	async apply(
		showId: string,
		expectedShowRevision: number,
		request: ProgrammingUpdateActionRequest,
	) {
		validateScopeId(showId, "show_id");
		integerAt(expectedShowRevision, "$expected.show_revision");
		const wireRequest = wireActionRequest(request);
		const body = encodeProgrammingUpdateActionRequest(wireRequest);
		const result = await this.post(
			this.showPath(showId, "actions"),
			body,
			expectedShowRevision,
		);
		const decoded = decodeProgrammingUpdateActionOutcome(
			result.value,
			showId,
			expectedShowRevision,
			wireRequest,
		);
		verifyRevisionEtag(result.response, decoded.show_revision);
		return programmingUpdateActionOutcome(decoded);
	}

	async loadSettings(deskId: string) {
		validateScopeId(deskId, "desk_id");
		const result = await this.request(this.settingsPath(deskId), {
			headers: this.headers(),
		});
		return programmingUpdateSettingsProjection(
			decodeProgrammingUpdateSettingsProjection(result.value, deskId),
		);
	}

	async saveSettings(
		deskId: string,
		settings: import("./types").UpdateSettings,
	) {
		validateScopeId(deskId, "desk_id");
		const body = encodeProgrammingUpdateSettings(wireSettings(settings));
		const result = await this.request(this.settingsPath(deskId), {
			method: "PUT",
			headers: this.headers(true),
			body: JSON.stringify(body),
		});
		return programmingUpdateSettingsProjection(
			decodeProgrammingUpdateSettingsProjection(result.value, deskId),
		);
	}

	private post(path: string, body: unknown, revision?: number) {
		return this.request(path, {
			method: "POST",
			headers: this.headers(true, revision),
			body: JSON.stringify(body),
		});
	}

	private async request(path: string, init: RequestInit) {
		let response: Response;
		try {
			response = await this.fetchImplementation(path, init);
		} catch (reason) {
			throw unavailableError(reason);
		}
		const value = await responseValue(response);
		if (response.status !== 200)
			throw new WireValidationError("$.status", "HTTP 200", response.status);
		return { response, value };
	}

	private headers(json = false, revision?: number) {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
		});
		if (json) headers.set("content-type", "application/json");
		if (revision != null) headers.set("if-match", `"${revision}"`);
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}

	private showPath(showId: string, operation: string) {
		const scope = `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}`;
		return `${scope}/programming-update/${operation}`;
	}

	private settingsPath(deskId: string) {
		const scope = `${this.baseUrl}/api/v2/desks/${encodeURIComponent(deskId)}`;
		return `${scope}/programming-update/settings`;
	}
}

async function responseValue(response: Response): Promise<unknown> {
	const text = await response.text();
	let value: unknown;
	try {
		value = text ? JSON.parse(text) : null;
	} catch {
		if (response.ok)
			throw new WireValidationError("$", "valid JSON response", text);
		throw fallbackError(response, text);
	}
	if (response.ok) return value;
	let error: ReturnType<typeof decodeProgrammingUpdateErrorResponse>;
	try {
		error = decodeProgrammingUpdateErrorResponse(value);
	} catch {
		throw fallbackError(response, text);
	}
	verifyErrorStatus(response.status, error.kind);
	verifyErrorEtag(response, error.current_show_revision ?? null);
	throw new ProgrammingUpdateHttpError(
		error.error,
		error.kind,
		response.status,
		error.current_object_revision ?? null,
		error.current_show_revision ?? null,
		error.retryable,
	);
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

function verifyErrorEtag(response: Response, revision: number | null) {
	const actual = response.headers.get("etag");
	if (revision == null) {
		if (actual != null)
			throw new WireValidationError("$.headers.etag", "no ETag", actual);
		return;
	}
	verifyRevisionEtag(response, revision);
}

function verifyErrorStatus(status: number, kind: ProgrammingUpdateErrorKind) {
	const expected = statusForKind(kind);
	if (status !== expected)
		throw new WireValidationError(
			"$.kind",
			`${kind} error for HTTP ${expected}`,
			`HTTP ${status}`,
		);
}

function statusForKind(kind: ProgrammingUpdateErrorKind) {
	if (kind === "invalid") return 400;
	if (kind === "unauthorized") return 401;
	if (kind === "forbidden") return 403;
	if (kind === "not_found") return 404;
	if (kind === "conflict") return 409;
	if (kind === "unavailable") return 503;
	return 500;
}

function validateScopeId(value: string, field: string) {
	scopedUuidAt(value, `$scope.${field}`);
}

function unavailableError(reason: unknown) {
	return new ProgrammingUpdateHttpError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		null,
		true,
	);
}

function fallbackError(response: Response, text: string) {
	return new ProgrammingUpdateHttpError(
		text || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number): ProgrammingUpdateErrorKind {
	if (status === 400) return "invalid";
	if (status === 401) return "unauthorized";
	if (status === 403) return "forbidden";
	if (status === 404) return "not_found";
	if (status === 409) return "conflict";
	if (status === 503) return "unavailable";
	return "internal";
}
