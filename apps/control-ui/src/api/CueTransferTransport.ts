import type {
	CueTransferActionRequest,
	CueTransferConflictRepair,
	CueTransferTransport,
} from "../features/cueTransfer/contracts";
import { CueTransferTransportError } from "../features/cueTransfer/contracts";
import {
	type CueTransferErrorKind,
	decodeCueTransferActionOutcome,
	decodeCueTransferErrorResponse,
	encodeCueTransferActionRequest,
} from "./cueTransferWire";
import {
	decodeProgrammingCommandLine,
	programmingUuidAt,
} from "./programmingWireProjection";
import { HttpShowObjectSnapshotTransport } from "./ShowObjectSnapshotTransport";
import { WireValidationError } from "./wireValidation";

export interface CueTransferTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

/** Strict action-only HTTP adapter; construction performs no network work. */
export class HttpCueTransferTransport
	implements CueTransferTransport, CueTransferConflictRepair
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly showObjects: HttpShowObjectSnapshotTransport;

	constructor(private readonly options: CueTransferTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/+$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.showObjects = new HttpShowObjectSnapshotTransport({
			...options,
			fetch: this.fetchImplementation,
		});
	}

	async apply(
		showId: string,
		expectedShowRevision: number,
		request: CueTransferActionRequest,
	) {
		programmingUuidAt(showId, "$.scope.show_id");
		const response = await this.fetchResponse(
			`${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/cues/transfer`,
			{
				method: "POST",
				headers: this.headers(expectedShowRevision),
				body: JSON.stringify(encodeCueTransferActionRequest(request)),
			},
		);
		const value = await responseValue(response);
		const outcome = decodeCueTransferActionOutcome(
			value,
			request,
			showId,
			expectedShowRevision,
		);
		verifyRevisionEtag(response, outcome.showRevision);
		return outcome;
	}

	loadCueLists: CueTransferConflictRepair["loadCueLists"] = (showId) =>
		this.showObjects.collection(showId, "cue_list");

	async loadCommandLine(deskId: string) {
		programmingUuidAt(deskId, "$.scope.desk_id");
		const response = await this.fetchImplementation(
			`${this.baseUrl}/api/v2/desks/${encodeURIComponent(deskId)}/command-line`,
			{ headers: this.authHeaders() },
		);
		const value = await exactReadValue(response);
		const commandLine = decodeProgrammingCommandLine(value);
		verifyRevisionEtag(response, commandLine.revision);
		return commandLine;
	}

	private headers(revision: number) {
		const headers = this.authHeaders();
		headers.set("content-type", "application/json");
		headers.set("if-match", `"${revision}"`);
		return headers;
	}

	private authHeaders() {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}

	private async fetchResponse(url: string, init: RequestInit) {
		try {
			return await this.fetchImplementation(url, init);
		} catch (reason) {
			throw new CueTransferTransportError(
				reason instanceof Error ? reason.message : String(reason),
				0,
				null,
				null,
				true,
			);
		}
	}
}

async function exactReadValue(response: Response): Promise<unknown> {
	const text = await response.text();
	if (!response.ok)
		throw new Error(text || `${response.status} ${response.statusText}`);
	try {
		return JSON.parse(text);
	} catch {
		throw new WireValidationError("$", "JSON response", text);
	}
}

async function responseValue(response: Response): Promise<unknown> {
	const text = await response.text();
	let value: unknown;
	try {
		value = text ? JSON.parse(text) : null;
	} catch {
		throw fallbackError(response, text);
	}
	if (response.ok) return value;
	let error: ReturnType<typeof decodeCueTransferErrorResponse>;
	try {
		error = decodeCueTransferErrorResponse(value);
	} catch {
		throw fallbackError(response, text);
	}
	verifyErrorStatus(response.status, error.kind);
	verifyErrorEtag(response, error.currentRevision);
	throw new CueTransferTransportError(
		error.error,
		response.status,
		error.currentRevision,
		error.currentRelatedRevision,
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
	if (revision == null) {
		if (response.headers.get("etag") != null)
			throw new WireValidationError(
				"$.headers.etag",
				"no ETag",
				response.headers.get("etag"),
			);
		return;
	}
	verifyRevisionEtag(response, revision);
}

function verifyErrorStatus(status: number, kind: CueTransferErrorKind) {
	const expected = statusForKind(kind);
	if (status !== expected)
		throw new WireValidationError(
			"$.kind",
			`${kind} error for HTTP ${expected}`,
			`HTTP ${status}`,
		);
}

function statusForKind(kind: CueTransferErrorKind) {
	if (kind === "invalid") return 400;
	if (kind === "unauthorized") return 401;
	if (kind === "forbidden") return 403;
	if (kind === "not_found") return 404;
	if (kind === "conflict") return 409;
	if (kind === "unavailable") return 503;
	return 500;
}

function fallbackError(response: Response, body: string) {
	return new CueTransferTransportError(
		body || `${response.status} ${response.statusText}`,
		response.status,
		null,
		null,
		response.status >= 500,
	);
}
