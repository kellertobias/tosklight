import type {
	CommandAcceptedAction,
	CommandChoiceOption,
	CommandChoiceOptionId,
	CommandHttpSource,
	CommandLineChangedEvent,
	CommandLineResponse,
	CommandOperationOutcome,
	CommandOperationResponse,
	CommandTarget,
	CueMoveCopyChoice,
	CueMoveCopyChoiceType,
	CueTransferOperation,
} from "./generated/light-wire";

const COMMAND_TARGETS = enumSet<CommandTarget>({ FIXTURE: true, GROUP: true });
const ACCEPTED_ACTIONS = enumSet<CommandAcceptedAction>({
	edited: true,
	executed: true,
	cleared_command_line: true,
	cleared_preload: true,
	cleared_selection: true,
	cleared_values: true,
	undone: true,
	no_change: true,
	preload_entered: true,
	preload_committed: true,
	shift_pressed: true,
	shift_released: true,
	ignored_release: true,
});
const CHOICE_OPTION_IDS = enumSet<CommandChoiceOptionId>({
	plain: true,
	status: true,
});
const CHOICE_TYPES = enumSet<CueMoveCopyChoiceType>({ cue_move_copy: true });
const CUE_TRANSFER_OPERATIONS = enumSet<CueTransferOperation>({
	copy: true,
	move: true,
});
const HTTP_SOURCES = enumSet<CommandHttpSource>({
	http: true,
	http_key: true,
});
const OUTCOMES = enumSet<CommandOperationOutcome["outcome"]>({
	accepted: true,
	choice_required: true,
	rejected: true,
});
const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type JsonObject = Record<string, unknown>;

function enumSet<T extends string>(members: Record<T, true>): ReadonlySet<T> {
	return new Set(Object.keys(members) as T[]);
}

/** A decoded wire value did not match the checked-in transport contract. */
export class WireValidationError extends TypeError {
	constructor(
		readonly path: string,
		expected: string,
		actual: unknown,
	) {
		super(`${path}: expected ${expected}; received ${describe(actual)}`);
		this.name = "WireValidationError";
	}
}

function describe(value: unknown): string {
	if (value === null) return "null";
	if (Array.isArray(value)) return "array";
	if (typeof value === "number" && !Number.isFinite(value)) return String(value);
	return typeof value;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}

function objectAt(value: unknown, path: string): JsonObject {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		return invalid(path, "an object", value);
	}
	return value as JsonObject;
}

function stringAt(value: unknown, path: string): asserts value is string {
	if (typeof value !== "string") invalid(path, "a string", value);
}

function booleanAt(value: unknown, path: string): asserts value is boolean {
	if (typeof value !== "boolean") invalid(path, "a boolean", value);
}

function unsignedIntegerAt(
	value: unknown,
	path: string,
): asserts value is number {
	if (!Number.isSafeInteger(value) || (value as number) < 0) {
		invalid(path, "a non-negative safe integer", value);
	}
}

function uuidAt(value: unknown, path: string): asserts value is string {
	if (typeof value !== "string" || !UUID_PATTERN.test(value)) {
		invalid(path, "a hyphenated UUID", value);
	}
}

function enumAt<T extends string>(
	value: unknown,
	path: string,
	values: ReadonlySet<T>,
): asserts value is T {
	if (typeof value !== "string" || !values.has(value as T)) {
		invalid(path, `one of ${[...values].join(", ")}`, value);
	}
}

function optionalNullableStringAt(
	object: JsonObject,
	key: string,
	path: string,
): void {
	if (!(key in object) || object[key] === null) return;
	stringAt(object[key], `${path}.${key}`);
}

function choiceOptionAt(
	value: unknown,
	path: string,
): asserts value is CommandChoiceOption {
	const option = objectAt(value, path);
	enumAt(option.id, `${path}.id`, CHOICE_OPTION_IDS);
	stringAt(option.label, `${path}.label`);
	stringAt(option.command, `${path}.command`);
}

function cueMoveCopyChoiceAt(
	value: unknown,
	path: string,
): asserts value is CueMoveCopyChoice {
	const choice = objectAt(value, path);
	enumAt(choice.type, `${path}.type`, CHOICE_TYPES);
	enumAt(
		choice.operation,
		`${path}.operation`,
		CUE_TRANSFER_OPERATIONS,
	);
	stringAt(choice.command, `${path}.command`);
	if (!Array.isArray(choice.options)) {
		invalid(`${path}.options`, "an array", choice.options);
	}
	choice.options.forEach((option, index) =>
		choiceOptionAt(option, `${path}.options[${index}]`),
	);
	stringAt(choice.cancel_label, `${path}.cancel_label`);
}

function commandLineAt(
	value: unknown,
	path: string,
): asserts value is CommandLineResponse {
	const commandLine = objectAt(value, path);
	stringAt(commandLine.text, `${path}.text`);
	enumAt(commandLine.target, `${path}.target`, COMMAND_TARGETS);
	booleanAt(commandLine.pristine, `${path}.pristine`);
	unsignedIntegerAt(commandLine.revision, `${path}.revision`);
	if (commandLine.pending_choice !== null) {
		cueMoveCopyChoiceAt(
			commandLine.pending_choice,
			`${path}.pending_choice`,
		);
	}
}

function operationOutcomeAt(
	value: unknown,
	path: string,
): asserts value is CommandOperationOutcome {
	const outcome = objectAt(value, path);
	enumAt(outcome.outcome, `${path}.outcome`, OUTCOMES);
	switch (outcome.outcome) {
		case "accepted":
			enumAt(outcome.action, `${path}.action`, ACCEPTED_ACTIONS);
			if (outcome.applied !== undefined && outcome.applied !== null) {
				unsignedIntegerAt(outcome.applied, `${path}.applied`);
			}
			optionalNullableStringAt(outcome, "warning", path);
			break;
		case "choice_required":
			cueMoveCopyChoiceAt(outcome.pending_choice, `${path}.pending_choice`);
			break;
		case "rejected":
			stringAt(outcome.error, `${path}.error`);
			break;
	}
}

/** Decode an authoritative v2 command-line projection. */
export function decodeCommandLineResponse(value: unknown): CommandLineResponse {
	commandLineAt(value, "$");
	return value as CommandLineResponse;
}

/** Decode the discriminated outcome shared by v2 mutating operations. */
export function decodeCommandOperationOutcome(
	value: unknown,
): CommandOperationOutcome {
	operationOutcomeAt(value, "$");
	return value as CommandOperationOutcome;
}

/** Decode a complete v2 mutating operation response. */
export function decodeCommandOperationResponse(
	value: unknown,
): CommandOperationResponse {
	const response = objectAt(value, "$");
	stringAt(response.request_id, "$.request_id");
	operationOutcomeAt(value, "$");
	commandLineAt(response.command_line, "$.command_line");
	return value as CommandOperationResponse;
}

/** Decode the typed v2 command-line compatibility event payload. */
export function decodeCommandLineChangedEvent(
	value: unknown,
): CommandLineChangedEvent {
	const event = objectAt(value, "$");
	uuidAt(event.desk_id, "$.desk_id");
	uuidAt(event.session_id, "$.session_id");
	uuidAt(event.user_id, "$.user_id");
	stringAt(event.text, "$.text");
	enumAt(event.target, "$.target", COMMAND_TARGETS);
	booleanAt(event.pristine, "$.pristine");
	unsignedIntegerAt(event.revision, "$.revision");
	enumAt(event.source, "$.source", HTTP_SOURCES);
	optionalNullableStringAt(event, "request_id", "$");
	if (event.redacted !== undefined) {
		booleanAt(event.redacted, "$.redacted");
	}
	return value as CommandLineChangedEvent;
}
