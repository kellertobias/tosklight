import { formatErrorDetails } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";

export type ShowLoadPhase = "catalogue" | "revisions" | "prepare" | "load";
type Task = (current: () => boolean) => Promise<void>;

/** UI lifetime only: invalidating a read ignores its result, it does not abort the server. */
export function useShowLoadOperation() {
	const mounted = useRef(true);
	const generation = useRef(0);
	const active = useRef<ShowLoadPhase | null>(null);
	const retry = useRef<(() => void) | null>(null);
	const [phase, setPhase] = useState<ShowLoadPhase | null>("catalogue");
	const [error, setError] = useState("");
	useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
			generation.current++;
			active.current = null;
		};
	}, []);
	async function run(next: ShowLoadPhase, task: Task) {
		if (active.current || !mounted.current) return;
		const token = ++generation.current;
		const current = () => mounted.current && generation.current === token;
		active.current = next;
		setPhase(next);
		setError("");
		retry.current = () => {
			void run(next, task);
		};
		try {
			await task(current);
		} catch (reason) {
			if (current()) setError(formatErrorDetails(reason));
		} finally {
			if (current()) {
				active.current = null;
				setPhase(null);
			}
		}
	}
	function cancelRead() {
		if (active.current === "load" || active.current === "prepare") return false;
		generation.current++;
		active.current = null;
		retry.current = null;
		if (mounted.current) {
			setPhase(null);
			setError("");
		}
		return true;
	}
	return {
		phase,
		error,
		run,
		cancelRead,
		retry: () => retry.current?.(),
		committing: phase === "load" || phase === "prepare",
		isBusy: () => active.current !== null,
	};
}
