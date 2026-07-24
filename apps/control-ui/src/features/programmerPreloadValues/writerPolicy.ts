import type { ProgrammerPreloadValuesStore } from "./store";

export function preloadValuesReadinessError(
	store: ProgrammerPreloadValuesStore,
	expectedScope: number,
) {
	if (!store.isScopeCurrent(expectedScope))
		return new Error("The Preload Programmer values scope has been replaced");
	const state = store.getSnapshot();
	if (state.repairRequired)
		return new Error(
			"Preload Programmer values authority is being repaired; retry when it is ready",
		);
	if (state.status === "loading")
		return new Error(
			"Authoritative Preload Programmer values are still loading",
		);
	if (state.status !== "ready" || !state.projection)
		return new Error("Authoritative Preload Programmer values are unavailable");
	return null;
}

export function preloadValuesError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
