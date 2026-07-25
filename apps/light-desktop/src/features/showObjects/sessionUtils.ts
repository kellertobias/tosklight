export function clearSessionTimers(
	reconnect: ReturnType<typeof globalThis.setTimeout> | null,
	hydration: ReturnType<typeof globalThis.setTimeout> | null,
) {
	if (reconnect != null) globalThis.clearTimeout(reconnect);
	if (hydration != null) globalThis.clearTimeout(hydration);
}

export function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}
