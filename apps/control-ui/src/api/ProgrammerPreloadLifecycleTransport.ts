import type {
	ProgrammerPreloadLifecycleRequest,
	ProgrammerPreloadLifecycleScope,
	ProgrammerPreloadLifecycleTransport,
} from "../features/programmerPreloadLifecycle/contracts";
import {
	ProgrammerPreloadLifecycleTransportError,
} from "../features/programmerPreloadLifecycle/contracts";
import {
	decodeProgrammerPreloadLifecycleErrorResponse,
	decodeProgrammerPreloadLifecycleOutcome,
	encodeProgrammerPreloadLifecycleRequest,
} from "./programmerPreloadLifecycleWire";
import { programmerValuesUuidAt } from "./programmerValuesWireProjection";

export interface HttpProgrammerPreloadLifecycleTransportOptions {
	baseUrl: string;
	sessionToken: string;
	authenticatedUserId: string;
	authenticatedDeskId: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
}

/** Action-only adapter: construction performs no request or subscription. */
export class HttpProgrammerPreloadLifecycleTransport
	implements ProgrammerPreloadLifecycleTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(
		private readonly options: HttpProgrammerPreloadLifecycleTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		programmerValuesUuidAt(
			options.authenticatedUserId,
			"$.authenticatedUserId",
		);
		programmerValuesUuidAt(options.authenticatedDeskId, "$.authenticatedDeskId");
	}

	async applyAction(
		scope: ProgrammerPreloadLifecycleScope,
		request: ProgrammerPreloadLifecycleRequest,
	) {
		validateScope(
			scope,
			this.options.authenticatedUserId,
			this.options.authenticatedDeskId,
		);
		const body = JSON.stringify(encodeProgrammerPreloadLifecycleRequest(request));
		const headers = this.headers();
		headers.set("content-type", "application/json");
		const path = `/api/v2/users/${encodeURIComponent(scope.userId)}/programmer-preload/actions`;
		let response: Response;
		try {
			response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
				method: "POST",
				headers,
				body,
			});
		} catch (reason) {
			throw unavailableError(reason);
		}
		return decodeProgrammerPreloadLifecycleOutcome(
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
			const error = decodeProgrammerPreloadLifecycleErrorResponse(value);
			throw new ProgrammerPreloadLifecycleTransportError(
				error.error,
				error.kind,
				response.status,
				error.currentRevision,
				error.currentRelatedRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof ProgrammerPreloadLifecycleTransportError)
				throw reason;
			throw fallbackError(response, text);
		}
	}
}

function validateScope(
	scope: ProgrammerPreloadLifecycleScope,
	authenticatedUserId: string,
	authenticatedDeskId: string,
) {
	programmerValuesUuidAt(scope.showId, "$.scope.showId");
	const userId = programmerValuesUuidAt(scope.userId, "$.scope.userId");
	const deskId = programmerValuesUuidAt(scope.deskId, "$.scope.deskId");
	if (userId.toLowerCase() !== authenticatedUserId.toLowerCase())
		throw new ProgrammerPreloadLifecycleTransportError(
			"Preload lifecycle scope does not match the authenticated user",
			"forbidden",
			403,
			null,
			null,
			false,
		);
	if (deskId.toLowerCase() !== authenticatedDeskId.toLowerCase())
		throw new ProgrammerPreloadLifecycleTransportError(
			"Preload lifecycle scope does not match the authenticated desk",
			"forbidden",
			403,
			null,
			null,
			false,
		);
}

function unavailableError(reason: unknown) {
	return new ProgrammerPreloadLifecycleTransportError(
		reason instanceof Error ? reason.message : String(reason),
		"unavailable",
		0,
		null,
		null,
		true,
	);
}

function fallbackError(response: Response, body: string) {
	return new ProgrammerPreloadLifecycleTransportError(
		body || `${response.status} ${response.statusText}`,
		kindForStatus(response.status),
		response.status,
		null,
		null,
		response.status >= 500,
	);
}

function kindForStatus(status: number) {
	if (status === 401) return "unauthorized" as const;
	if (status === 403) return "forbidden" as const;
	if (status === 404) return "not_found" as const;
	if (status === 409) return "conflict" as const;
	if (status === 503) return "unavailable" as const;
	return status >= 500 ? ("internal" as const) : ("invalid" as const);
}
