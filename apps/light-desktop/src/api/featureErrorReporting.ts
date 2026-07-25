import type { Dispatch, SetStateAction } from "react";

type FeatureErrorLane = "session" | "mutation";
type ErrorEntry = { message: string; order: number };

export function createFeatureErrorGroup(
	setError: Dispatch<SetStateAction<string | null>>,
) {
	const entries: Record<FeatureErrorLane, ErrorEntry | null> = {
		session: null,
		mutation: null,
	};
	let order = 0;
	let displayed: string | null = null;

	const report = (lane: FeatureErrorLane, error: Error | null) => {
		const previous = displayed;
		entries[lane] = error
			? { message: error.message, order: ++order }
			: null;
		const next = Object.values(entries)
			.filter((entry) => entry !== null)
			.sort((left, right) => right.order - left.order)[0]?.message ?? null;
		displayed = next;
		if (error) setError(next);
		else setError((current) => (current === previous ? next : current));
	};

	return {
		reportSession: (error: Error | null) => report("session", error),
		reportMutation: (error: Error | null) => report("mutation", error),
	};
}
