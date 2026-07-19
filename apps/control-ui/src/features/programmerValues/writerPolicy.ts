import type { ProgrammerValuesStore } from "./store";

/** Refuses optimism and dispatch unless the scoped values authority is usable. */
export function programmerValuesReadinessError(
	store: ProgrammerValuesStore,
	expectedScope: number,
) {
	if (!store.isScopeCurrent(expectedScope))
		return new Error("The Programmer values scope has been replaced");
	const state = store.getSnapshot();
	if (state.repairRequired)
		return new Error(
			"Programmer values authority is being repaired; retry when it is ready",
		);
	if (state.status === "loading")
		return new Error("Authoritative Programmer values are still loading");
	if (state.status !== "ready" || !state.projection)
		return new Error("Authoritative Programmer values are unavailable");
	return null;
}

export function isReplayableValuesError(reason: unknown) {
	if (!reason || typeof reason !== "object") return true;
	if ("retryable" in reason)
		return (reason as { retryable?: unknown }).retryable === true;
	return !("status" in reason);
}

export function requiresValuesAuthorityRepair(reason: unknown) {
	if (!reason || typeof reason !== "object") return true;
	const status =
		"status" in reason ? (reason as { status?: unknown }).status : null;
	return (
		status === null ||
		status === 408 ||
		status === 409 ||
		status === 423 ||
		(typeof status === "number" && status >= 500)
	);
}

export function programmerValuesError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
