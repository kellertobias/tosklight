import { useEffect, useRef } from "react";

interface PollingResourceOptions<T> {
	enabled: boolean;
	intervalMillis: number;
	load: () => Promise<T>;
	onValue: (value: T) => void;
	refreshKey?: unknown;
	onError?: (error: unknown) => void;
}

/**
 * Transitional polling lifecycle for projections that have not moved to retained streams yet.
 * Hidden consumers do no work and a slow request never accumulates overlapping refreshes.
 */
export function usePollingResource<T>({
	enabled,
	intervalMillis,
	load,
	onValue,
	refreshKey,
	onError,
}: PollingResourceOptions<T>) {
	const callbacks = useRef({ load, onValue, onError });
	callbacks.current = { load, onValue, onError };

	useEffect(() => {
		if (!enabled) return;
		let cancelled = false;
		let inFlight = false;
		const refresh = async () => {
			if (inFlight) return;
			inFlight = true;
			try {
				const value = await callbacks.current.load();
				if (!cancelled) callbacks.current.onValue(value);
			} catch (error) {
				if (!cancelled) callbacks.current.onError?.(error);
			} finally {
				inFlight = false;
			}
		};
		void refresh();
		const timer = window.setInterval(() => void refresh(), intervalMillis);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [enabled, intervalMillis, refreshKey]);
}
