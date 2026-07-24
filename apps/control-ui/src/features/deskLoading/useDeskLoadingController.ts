import { useCallback, useRef, useState } from "react";
import type { DeskLoadingState } from "./DeskLoadingState";

/**
 * Tracks overlapping desk-wide operations by token.
 *
 * Show-open responses and their matching server events can hydrate concurrently; the loading
 * surface remains until both owners finish, without an older completion clearing a newer phase.
 */
export function useDeskLoadingController() {
	const [deskLoading, setDeskLoading] = useState<DeskLoadingState | null>(null);
	const generation = useRef(0);
	const operations = useRef(new Map<number, DeskLoadingState>());
	const beginDeskLoading = useCallback((title: string, detail: string) => {
		const operationId = ++generation.current;
		const loading = { operationId, title, detail };
		operations.current.set(operationId, loading);
		setDeskLoading(loading);
		return operationId;
	}, []);
	const finishDeskLoading = useCallback((operationId: number) => {
		if (!operations.current.delete(operationId)) return;
		const remaining = [...operations.current.values()];
		setDeskLoading(remaining.at(-1) ?? null);
	}, []);
	return { deskLoading, beginDeskLoading, finishDeskLoading };
}
