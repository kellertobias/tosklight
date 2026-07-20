import { useLayoutEffect, useRef } from "react";

interface Stoppable {
	stop(): void;
}

/**
 * Dispose a memoized authority after its final layout-effect cleanup.
 *
 * React StrictMode immediately repeats a mount effect. Deferring disposal by one
 * microtask lets that repeated setup retain the same authority, while replaced
 * or genuinely unmounted instances are still stopped before later tasks run.
 */
export function useStrictModeSafeStop(target: Stoppable | null) {
	const generations = useRef(new WeakMap<Stoppable, number>());
	useLayoutEffect(() => {
		if (!target) return;
		const generation = (generations.current.get(target) ?? 0) + 1;
		generations.current.set(target, generation);
		return () => {
			globalThis.queueMicrotask(() => {
				if (generations.current.get(target) !== generation) return;
				generations.current.delete(target);
				target.stop();
			});
		};
	}, [target]);
}
