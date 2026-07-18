import { useCallback, useRef } from "react";

export function useStableCallback<Arguments extends unknown[], Result>(
	callback: (...arguments_: Arguments) => Result,
) {
	const callbackRef = useRef(callback);
	callbackRef.current = callback;
	return useCallback(
		(...arguments_: Arguments) => callbackRef.current(...arguments_),
		[],
	);
}
