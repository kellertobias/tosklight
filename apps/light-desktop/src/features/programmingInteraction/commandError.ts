import { formatErrorDetails } from "@tosklight/ui";

/** Scoped command feedback. Null retires the current error on the next edit. */
export const COMMAND_ERROR_EVENT = "light:command-error";
export type CommandErrorFeedback = {
	message: string;
	kind: "operator" | "unexpected";
	command?: string;
};

const UNEXPECTED =
	/could not be synchronized|unexpected|invalid server response|internal (?:server|error)|network|failed to fetch|task failed|inconsistent authoritative|runtime snapshot|database|panic|unauthorized|forbidden|authentication|authorization|permission denied|access denied|session expired/i;
const OPERATOR_REJECTION =
	/\b(?:fixture|group|preset|cue|cuelist|playback|target|level|percentage|selection|syntax|command|record|update|FixAT)\b.*(?:invalid|not recorded|not found|does not exist|not patched|must|requires|unsupported|cannot|not a|changed|empty|already|out of range)|\bno selection\b/i;

const REVERSED_OPERATOR_REJECTION =
	/^(?:invalid|unsupported)\s+(?:fixture|group|preset|cue|cuelist|playback|target|level|percentage|selection|command syntax)\b/i;
const SELECTION_REQUIRED =
	/^(?:- requires (?:a fixture selection|fixture selections on both sides)|Programmer Align requires a non-empty selection)$/i;

// These exact messages are emitted by the command parser, not the runtime.
const TRAILING_TOKEN_REJECTION =
	/^unexpected tokens after (?:level|selection|subset|playback address|playback selection|recording target|Cue transfer address|Cue deletion address|BPM value|Cue number|cue target|Link source|Link destination)$/i;

const RECORD_GRAMMAR_REJECTIONS = new Set([
	"record + and record - currently require group or pbk ... cue targets",
	"record + and record - require an explicit cue target",
]);

const TRANSFER_GRAMMAR_REJECTIONS = new Set([
	"preset move and copy require <family> preset <number> on both sides",
	"preset move and copy require preset before the number",
]);

// Explicit input grammar errors; never infer operator errors from arbitrary expected/required text.
const INPUT_GRAMMAR_REJECTIONS = new Set([
	"time and delay require seconds",
	"time and delay must be within 0-86400 seconds",
	"time decimal requires digits after the dot",
	"timing ranges are not supported",
	"expected a fixture number",
	"at requires a level",
	"expected offset or + before the subset offset",
	"expected a fixture reference before +",
	"expected + between fixture ranges",
	"expected a fixture reference after +",
	"expected set dynamic <pool number>",
	"expected set cuelist <positive pool number>",
	"expected set group <group-number>",
	"set requires an editable value or object",
	"assign requires a source before at",
	"expected assign group <number> at pbk <address>",
	"expected assign <cuelist|dynamic|macro|timecode> <number> at pbk <address>",
	"dmx requires one universe.address",
	"div requires a positive number",
	"dynamic requires a pool number",
	"expected dynamic <number>, off, size/speed/phase at, or phase helpers",
	"dot requires another cue part",
	"link requires at cue <destination>",
	"link requires cue or set playback cue source",
	"at requires a bpm value",
	"expected pbk playback address",
	"expected spd grp <1-5> at <bpm | +/- bpm | spd grp <1-5>>",
	"expected cue, cuelist, or pbk recording target",
	"expected [set] macro|timecode <positive pool number> [+|-], or set cuelist <positive pool number>",
	"expected cuelist <number> at spd grp <1-5>",
	"expected dynamic <number> at spd grp <1-5>",
]);

function isUnexpectedFailure(message: string): boolean {
	return (
		UNEXPECTED.test(message) &&
		!TRAILING_TOKEN_REJECTION.test(message.replace(/^invalid data:\s*/i, ""))
	);
}

/** History carries plain feedback, so unrecognised failures retain full diagnostics. */
export function commandErrorFeedback(
	detail: string | CommandErrorFeedback,
): CommandErrorFeedback {
	if (typeof detail !== "string") return detail;
	const message = detail.replace(/^invalid data:\s*/i, "");
	return {
		message: detail,
		kind:
			!isUnexpectedFailure(detail) &&
			(OPERATOR_REJECTION.test(message) ||
				REVERSED_OPERATOR_REJECTION.test(message) ||
				SELECTION_REQUIRED.test(message) ||
				TRAILING_TOKEN_REJECTION.test(message) ||
				INPUT_GRAMMAR_REJECTIONS.has(message.toLowerCase()) ||
				TRANSFER_GRAMMAR_REJECTIONS.has(message.toLowerCase()) ||
				RECORD_GRAMMAR_REJECTIONS.has(message.toLowerCase()))
				? "operator"
				: "unexpected",
	};
}

export function commandErrorSummary(message: string): string {
	if (
		TRANSFER_GRAMMAR_REJECTIONS.has(
			message.replace(/^invalid data:\s*/i, "").toLowerCase(),
		) ||
		RECORD_GRAMMAR_REJECTIONS.has(
			message.replace(/^invalid data:\s*/i, "").toLowerCase(),
		)
	)
		return "Invalid Command";
	if (
		/\bno selection\b|\bselection\b.*(?:empty|required)|requires.*selection/i.test(
			message,
		)
	)
		return "No Selection";
	if (/\bpreset\b.*(?:does not exist|not found|not recorded)/i.test(message))
		return "Preset not Recorded";
	return message
		.split("\n")[0]
		.replace(/^invalid data:\s*/i, "")
		.slice(0, 120);
}

export function reportProgrammingCommandError(error: Error | null): void {
	if (!error) return;
	const status = "status" in error ? error.status : undefined;
	// Websocket command responses carry plain error text, without an HTTP status.
	// Only known operator messages may use that fallback; typed statuses win.
	const operator =
		!isUnexpectedFailure(error.message) &&
		(status === undefined
			? commandErrorFeedback(error.message).kind === "operator"
			: typeof status === "number" &&
				status >= 400 &&
				status < 500 &&
				status !== 401 &&
				status !== 403);
	window.dispatchEvent(
		new CustomEvent<CommandErrorFeedback>(COMMAND_ERROR_EVENT, {
			detail: {
				message: operator ? error.message : formatErrorDetails(error),
				kind: operator ? "operator" : "unexpected",
			},
		}),
	);
}

export function clearProgrammingCommandError(): void {
	window.dispatchEvent(
		new CustomEvent<null>(COMMAND_ERROR_EVENT, { detail: null }),
	);
}
