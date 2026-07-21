import type { ApiDriver, Session } from "./api";

export interface IntentHttpDependencies {
	fetch?: typeof globalThis.fetch;
	requestId?: () => string;
}

export function intentSession(api: ApiDriver): Session {
	if (!api.session) throw new Error("API session is not initialized");
	return api.session;
}

export function intentFetch(dependencies: IntentHttpDependencies) {
	return dependencies.fetch ?? globalThis.fetch.bind(globalThis);
}

export function intentRequestId(dependencies: IntentHttpDependencies) {
	return dependencies.requestId?.() ?? crypto.randomUUID();
}

export function intentUrl(api: ApiDriver, path: string) {
	return `${api.baseUrl.replace(/\/$/, "")}${path}`;
}

export function intentHeaders(session: Session) {
	return { authorization: `Bearer ${session.token}` };
}

export async function responseJson(response: Response, label: string) {
	try {
		return (await response.json()) as unknown;
	} catch {
		throw new Error(`${label} returned non-JSON HTTP ${response.status}`);
	}
}
