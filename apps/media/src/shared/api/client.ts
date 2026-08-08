// The transport. One place converts an HTTP exchange into either a typed value or a typed
// failure, so no feature ever branches on a status code or reads `error.message` to decide what
// happened.

import type { ApiErrorBody, CatalogView, Health, OutputView, UpdateLayer } from "./generated/media-wire";

/// Every failure a call site can see, including the ones that never reached the server.
export class ApiFailure extends Error {
	readonly code: string;
	readonly status: number;

	constructor(code: string, message: string, status: number) {
		super(message);
		this.name = "ApiFailure";
		this.code = code;
		this.status = status;
	}

	/** The server is not answering — a different situation from it answering "no". */
	get disconnected(): boolean {
		return this.status === 0;
	}

	/** A desk owns this output, so the web interface may not write to it right now. */
	get deskOwnsIt(): boolean {
		return this.code === "dmx-owns-this";
	}
}

const BASE = "/api/v2";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
	let response: Response;
	try {
		response = await fetch(`${BASE}${path}`, {
			...init,
			headers: init?.body ? { "content-type": "application/json" } : undefined,
		});
	} catch {
		throw new ApiFailure(
			"unreachable",
			"the Media Server is not answering; check that it is running",
			0,
		);
	}

	if (!response.ok) throw await failureOf(response);
	if (response.status === 204) return undefined as T;
	return (await response.json()) as T;
}

async function failureOf(response: Response): Promise<ApiFailure> {
	// A route answers with the typed error body. Anything else — a proxy, a crash page — still
	// has to become a failure an operator can read.
	try {
		const body = (await response.json()) as ApiErrorBody;
		if (typeof body?.code === "string" && typeof body?.message === "string") {
			return new ApiFailure(body.code, body.message, response.status);
		}
	} catch {
		// fall through to the generic failure
	}
	return new ApiFailure(
		"unexpected-response",
		`the server answered ${response.status}`,
		response.status,
	);
}

export const api = {
	health: () => request<Health>("/health"),
	catalog: () => request<CatalogView>("/catalog"),
	outputs: () => request<OutputView[]>("/outputs"),
	outputState: (output: string) => request<OutputView>(`/outputs/${output}/state`),

	/** An intent-shaped write: only the fields being changed travel. */
	updateLayer: (output: string, layer: number, update: UpdateLayer) =>
		request<OutputView>(`/outputs/${output}/layers/${layer}/update`, {
			method: "POST",
			body: JSON.stringify(update),
		}),

	/** A live-control action with no payload, exactly as the API exposes it. */
	resetLayer: (output: string, layer: number) =>
		request<void>(`/outputs/${output}/layers/${layer}/reset`),
};

export type MediaApi = typeof api;
