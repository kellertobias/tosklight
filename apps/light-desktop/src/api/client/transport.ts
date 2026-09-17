import type { LiveAction } from "../generated/light-wire";

export interface ClientTransport {
	request<T>(
		path: string,
		init?: RequestInit,
		authenticate?: boolean,
	): Promise<T>;
	blob(path: string, init?: RequestInit): Promise<Blob>;
	/**
	 * The authenticated response itself, whatever its status, for a caller that needs headers or
	 * a typed failure body alongside binary content.
	 */
	response?(path: string, init?: RequestInit): Promise<Response>;
	absoluteUrl(path: string): string;
}

export interface LiveClientTransport extends ClientTransport {
	currentDeskId(): string | null;
	sendAction(action: LiveAction, requestId?: string): Promise<unknown>;
}

export function jsonRequest(
	method: "POST" | "PUT",
	body: unknown,
): RequestInit {
	return {
		method,
		headers: { "content-type": "application/json" },
		body: JSON.stringify(body),
	};
}
