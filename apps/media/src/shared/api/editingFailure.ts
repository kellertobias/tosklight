import { useCallback, useEffect, useRef } from "react";
import { useToast } from "../../app/ToastContext";
import { ApiFailure } from "./client";

/** Keep normal refusals in the editor; after navigation, report through the surviving shell. */
export function useEditingFailure(setFailure: (failure: ApiFailure) => void) {
	const mounted = useRef(true);
	const { error: showError } = useToast();
	useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
		};
	}, []);
	return useCallback(
		(error: unknown) => {
			const failure = error instanceof ApiFailure
				? error
				: new ApiFailure("unexpected-error", String(error), 0);
			if (mounted.current) setFailure(failure);
			else showError(failure.message);
		},
		[setFailure, showError],
	);
}
