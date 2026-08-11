import { useSyncExternalStore } from "react";

export type SystemControlsTab =
	| "running"
	| "desk-state"
	| "active-programmers";

let requestedTab: SystemControlsTab = "running";
const listeners = new Set<() => void>();

export function requestSystemControlsTab(tab: SystemControlsTab) {
	if (requestedTab === tab) return;
	requestedTab = tab;
	for (const listener of listeners) listener();
}

export function useRequestedSystemControlsTab(): SystemControlsTab {
	return useSyncExternalStore(
		(listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		() => requestedTab,
		() => "running",
	);
}

export interface DeskStateDiagnostic {
	id: string;
	title: string;
	summary: string;
	action: string;
	detail?: string;
}
