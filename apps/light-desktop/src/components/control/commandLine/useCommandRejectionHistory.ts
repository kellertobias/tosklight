import { type Dispatch, type SetStateAction, useEffect, useRef } from "react";
import type { CommandHistoryEntry } from "../../../api/types";
import { useProgrammingInteractionStore } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { commandErrorFeedback } from "../../../features/programmingInteraction/commandError";

type UpdateError = (
	message: string | null,
	kind?: "operator" | "unexpected",
	command?: string,
) => void;

/** Retained feedback belongs to the command and scope that produced it, not later edits. */
export function useCommandRejectionEdits({
	commandError: message,
	commandErrorRevision: errorRevision,
	commandErrorCommand: rejectedCommand,
	setCommandError: updateError,
}: {
	commandError: string | null;
	commandErrorRevision: number;
	commandErrorCommand: string | undefined;
	setCommandError: UpdateError;
}) {
	const store = useProgrammingInteractionStore();
	const { showId, deskId, commandLine } = store.getSnapshot();
	const authorityScope = store.captureScope();
	const current = { store, showId, deskId, authorityScope, commandLine, errorRevision };
	const previous = useRef(current);
	useEffect(() => {
		const before = previous.current;
		previous.current = current;
		if (!message) return;
		const scopeChanged =
			before.store !== store || before.authorityScope !== authorityScope ||
			before.showId !== showId ||
			before.deskId !== deskId;
		const lostAuthority = before.commandLine !== null && commandLine === null;
		const edited = before.commandLine && commandLine && (
			before.commandLine.text !== commandLine.text ||
			before.commandLine.target !== commandLine.target ||
			before.commandLine.pristine !== commandLine.pristine
		);
		// An outcome may arrive before its matching projection. Neither that delivery nor
		// an unrelated revision/history update is another operator edit.
		const newRejection = before.errorRevision !== errorRevision;
		if (
			scopeChanged || lostAuthority ||
			(edited && !newRejection && commandLine?.text !== rejectedCommand)
		) updateError(null);
	}, [
		store, showId, deskId, authorityScope, commandLine, message,
		errorRevision, rejectedCommand, updateError,
	]);
}

/** Observe this desk's new OSC outcomes without replaying retained history after mounting. */
export function useOSCCommandRejections(
	history: readonly CommandHistoryEntry[],
	updateError: UpdateError,
	setCompleted: Dispatch<SetStateAction<boolean>>,
) {
	const store = useProgrammingInteractionStore();
	const { showId, deskId } = store.getSnapshot();
	const authorityScope = store.captureScope();
	const scope = useRef({ store, showId, deskId, authorityScope });
	const seenHistoryId = useRef(history[0]?.id);
	const mountedAt = useRef(Date.now());
	useEffect(() => {
		const latest = history[0];
		if (scope.current.store !== store || scope.current.authorityScope !== authorityScope ||
			scope.current.showId !== showId || scope.current.deskId !== deskId) {
			scope.current = { store, showId, deskId, authorityScope };
			seenHistoryId.current = latest?.id;
			mountedAt.current = Date.now();
			return;
		}
		if (!latest || latest.id === seenHistoryId.current) return;
		seenHistoryId.current = latest.id;
		if (
			(deskId && latest.desk_id !== deskId) ||
			latest.source !== "osc" ||
			Date.parse(latest.at) < mountedAt.current
		) return;
		if (latest.status === "rejected") {
			const feedback = commandErrorFeedback(latest.feedback);
			updateError(feedback.message, feedback.kind, latest.command);
			setCompleted(false);
		} else updateError(null);
	}, [history, store, showId, deskId, authorityScope, updateError, setCompleted]);
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
