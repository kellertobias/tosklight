import type { CommandHistoryItem } from "@tosklight/ui";
import { type Dispatch, type SetStateAction, useEffect, useRef } from "react";
import { commandErrorFeedback } from "../../../features/programmingInteraction/commandError";

type UpdateError = (
	message: string | null,
	kind?: "operator" | "unexpected",
	command?: string,
) => void;

/** Observe this desk's new OSC outcomes without replaying retained history after mounting. */
export function useOSCCommandRejections(
	history: readonly CommandHistoryItem[],
	updateError: UpdateError,
	setCompleted: Dispatch<SetStateAction<boolean>>,
) {
	const seenHistoryId = useRef(history[0]?.id);
	const mountedAt = useRef(Date.now());
	useEffect(() => {
		const latest = history[0];
		if (!latest || latest.id === seenHistoryId.current) return;
		seenHistoryId.current = latest.id;
		if (latest.source !== "osc" || Date.parse(latest.at) < mountedAt.current)
			return;
		if (latest.status === "rejected") {
			const feedback = commandErrorFeedback(latest.feedback);
			updateError(feedback.message, feedback.kind, latest.command);
			setCompleted(false);
		} else updateError(null);
	}, [history, updateError, setCompleted]);
}

/** Ordinary mistakes never open the larger history panel automatically. */
export function useUnexpectedCommandHistory(
	message: string | null,
	kind: "operator" | "unexpected",
	setOpen: Dispatch<SetStateAction<boolean>>,
) {
	useEffect(() => {
		if (message && kind === "unexpected") setOpen(true);
	}, [message, kind, setOpen]);
}
