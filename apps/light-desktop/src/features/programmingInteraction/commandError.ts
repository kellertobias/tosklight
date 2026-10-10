import { formatErrorDetails } from "@tosklight/ui";

/** Scoped command feedback. Null retires the current error on the next edit. */
export const COMMAND_ERROR_EVENT = "light:command-error";
export type CommandErrorFeedback = {
	message: string;
	kind: "operator" | "unexpected";
	command?: string;
};

const UNEXPECTED = /could not be synchronized|unexpected|invalid server response|internal (?:server|error)|network|failed to fetch|task failed|inconsistent authoritative|runtime snapshot|database|panic|unauthorized|forbidden|authentication|authorization|permission denied|access denied|session expired/i;
const OPERATOR_REJECTION = /\b(?:fixture|group|preset|cue|cuelist|playback|target|level|percentage|selection|syntax|command|record|update|FixAT)\b.*(?:invalid|not recorded|not found|does not exist|not patched|must|requires|unsupported|cannot|not a|changed|empty|already|out of range)|\bno selection\b/i;

const REVERSED_OPERATOR_REJECTION = /^(?:invalid|unsupported)\s+(?:fixture|group|preset|cue|cuelist|playback|target|level|percentage|selection|command syntax)\b/i;
const SELECTION_REQUIRED = /^(?:- requires (?:a fixture selection|fixture selections on both sides)|Programmer Align requires a non-empty selection)$/i;

/** History carries plain feedback, so unrecognised failures retain full diagnostics. */
export function commandErrorFeedback(
	detail: string | CommandErrorFeedback,
): CommandErrorFeedback {
	if (typeof detail !== "string") return detail;
	const message = detail.replace(/^invalid data:\s*/i, "");
	return {
		message: detail,
		kind: !UNEXPECTED.test(detail) &&
			(OPERATOR_REJECTION.test(message) ||
				REVERSED_OPERATOR_REJECTION.test(message) ||
				SELECTION_REQUIRED.test(message))
			? "operator"
			: "unexpected",
	};
}

export function commandErrorSummary(message: string): string {
	if (/\bno selection\b|\bselection\b.*(?:empty|required)|requires.*selection/i.test(message))
		return "No Selection";
	if (/\bpreset\b.*(?:does not exist|not found|not recorded)/i.test(message))
		return "Preset not Recorded";
	return message.split("\n")[0].replace(/^invalid data:\s*/i, "").slice(0, 120);
}

export function reportProgrammingCommandError(error: Error | null): void {
	if (!error) return;
	const status = "status" in error ? error.status : undefined;
	// Websocket command responses carry plain error text, without an HTTP status.
	// Only known operator messages may use that fallback; typed statuses win.
	const operator =
		!UNEXPECTED.test(error.message) &&
		(status === undefined
			? commandErrorFeedback(error.message).kind === "operator"
			: typeof status === "number" &&
				status >= 400 &&
				status < 500 &&
				status !== 401 &&
				status !== 403);
	window.dispatchEvent(new CustomEvent<CommandErrorFeedback>(COMMAND_ERROR_EVENT, {
		detail: {
			message: operator ? error.message : formatErrorDetails(error),
			kind: operator ? "operator" : "unexpected",
		},
	}));
}

export function clearProgrammingCommandError(): void {
	window.dispatchEvent(new CustomEvent<null>(COMMAND_ERROR_EVENT, { detail: null }));
}
