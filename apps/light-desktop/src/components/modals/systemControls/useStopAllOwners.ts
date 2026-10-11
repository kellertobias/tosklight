import { useRef, useState } from "react";

/** Confirmation acts on each captured authoritative owner once and retains partial failures. */
export function useStopAllOwners(ready: boolean, operations: (() => Promise<unknown>)[]) {
	const [stoppingAll, setStoppingAll] = useState(false);
	const [stopAllError, setStopAllError] = useState<string | null>(null);
	const pending = useRef(false);
	const stopAllRunning = async () => {
		if (pending.current) return false;
		if (!ready) {
			setStopAllError(
				"Running state changed before confirmation. Review the remaining items and try again.",
			);
			return false;
		}
		pending.current = true;
		setStoppingAll(true);
		setStopAllError(null);
		try {
			const outcomes = await Promise.allSettled(
				operations.map((operation) => operation()),
			);
			const failed = outcomes.filter(
				(outcome) => outcome.status === "rejected" || outcome.value === false,
			);
			if (failed.length) {
				setStopAllError(
					`${failed.length} running item${failed.length === 1 ? "" : "s"} could not be turned off. Review the remaining items and try again.`,
				);
				return false;
			}
			return true;
		} finally {
			pending.current = false;
			setStoppingAll(false);
		}
	};
	return {
		stoppingAll,
		stopAllError,
		stopAllRunning,
		clearStopAllError: () => setStopAllError(null),
	};
}
