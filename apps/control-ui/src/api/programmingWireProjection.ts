import type {
	CommandChoiceOption,
	CommandLineProjection,
	PendingCommandChoice,
	ProgrammingChange,
	ProgrammingProjection,
	SelectionExpression,
	SelectionProjection,
	SelectionReference,
	SelectionRule,
} from "../features/programmingInteraction/contracts";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	positiveIntegerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function plainProgrammingStringAt(value: unknown, path: string) {
	if (typeof value !== "string")
		throw new WireValidationError(path, "string", value);
	return value;
}

export function programmingUuidAt(value: unknown, path: string) {
	if (typeof value !== "string" || !UUID_PATTERN.test(value))
		throw new WireValidationError(path, "hyphenated UUID", value);
	return value;
}

function decodeChoiceOption(
	value: unknown,
	path: string,
): CommandChoiceOption {
	const option = recordAt(value, path);
	return {
		id: enumAt(option.id, `${path}.id`, ["plain", "status"]),
		label: plainProgrammingStringAt(option.label, `${path}.label`),
		command: plainProgrammingStringAt(option.command, `${path}.command`),
	};
}

function decodePendingChoice(value: unknown, path: string): PendingCommandChoice {
	const choice = recordAt(value, path);
	return {
		type: enumAt(choice.type, `${path}.type`, ["cue_move_copy"]),
		operation: enumAt(choice.operation, `${path}.operation`, ["copy", "move"]),
		command: plainProgrammingStringAt(choice.command, `${path}.command`),
		options: arrayAt(choice.options, `${path}.options`).map((option, index) =>
			decodeChoiceOption(option, `${path}.options[${index}]`),
		),
		cancelLabel: plainProgrammingStringAt(
			choice.cancel_label,
			`${path}.cancel_label`,
		),
	};
}

/** Decode only validated fields from an authoritative command-line projection. */
export function decodeProgrammingCommandLine(
	value: unknown,
	path = "$",
): CommandLineProjection {
	const commandLine = recordAt(value, path);
	if (!("pending_choice" in commandLine))
		throw new WireValidationError(
			`${path}.pending_choice`,
			"pending choice or null",
			undefined,
		);
	return {
		text: plainProgrammingStringAt(commandLine.text, `${path}.text`),
		target: enumAt(commandLine.target, `${path}.target`, ["FIXTURE", "GROUP"]),
		pristine: booleanAt(commandLine.pristine, `${path}.pristine`),
		revision: integerAt(commandLine.revision, `${path}.revision`),
		pendingChoice:
			commandLine.pending_choice === null
				? null
				: decodePendingChoice(
						commandLine.pending_choice,
						`${path}.pending_choice`,
					),
	};
}

function decodeSelectionRule(value: unknown, path: string): SelectionRule {
	const rule = recordAt(value, path);
	const type = enumAt(rule.type, `${path}.type`, [
		"all",
		"odd",
		"even",
		"every_nth",
	]);
	if (type !== "every_nth") return { type };
	return {
		type,
		n: positiveIntegerAt(rule.n, `${path}.n`),
		offset: integerAt(rule.offset, `${path}.offset`),
	};
}

function decodeSelectionReference(
	value: unknown,
	path: string,
): SelectionReference {
	const reference = recordAt(value, path);
	const type = enumAt(reference.type, `${path}.type`, [
		"fixture",
		"live_group",
		"remove_fixture",
		"remove_live_group",
	]);
	if (type === "fixture" || type === "remove_fixture")
		return {
			type,
			fixtureId: programmingUuidAt(
				reference.fixture_id,
				`${path}.fixture_id`,
			),
		};
	return {
		type,
		groupId: stringAt(reference.group_id, `${path}.group_id`),
	};
}

function decodeSelectionExpression(
	value: unknown,
	path: string,
): SelectionExpression {
	const expression = recordAt(value, path);
	const type = enumAt(expression.type, `${path}.type`, [
		"static",
		"live_group",
		"frozen_group",
		"playback_contents",
		"sources",
	]);
	if (type === "static") return { type };
	if (type === "live_group")
		return {
			type,
			groupId: stringAt(expression.group_id, `${path}.group_id`),
			rule: decodeSelectionRule(expression.rule, `${path}.rule`),
		};
	if (type === "frozen_group")
		return {
			type,
			groupId: stringAt(expression.group_id, `${path}.group_id`),
			sourceRevision: integerAt(
				expression.source_revision,
				`${path}.source_revision`,
			),
		};
	return {
		type,
		items: arrayAt(expression.items, `${path}.items`).map((item, index) =>
			decodeSelectionReference(item, `${path}.items[${index}]`),
		),
	};
}

function decodeSelection(
	value: unknown,
	path: string,
): SelectionProjection {
	const selection = recordAt(value, path);
	if (!("expression" in selection))
		throw new WireValidationError(
			`${path}.expression`,
			"selection expression or null",
			undefined,
		);
	return {
		selected: arrayAt(selection.selected, `${path}.selected`).map((id, index) =>
			programmingUuidAt(id, `${path}.selected[${index}]`),
		),
		expression:
			selection.expression === null
				? null
				: decodeSelectionExpression(
						selection.expression,
						`${path}.expression`,
					),
		revision: integerAt(selection.revision, `${path}.revision`),
		gestureOpen: booleanAt(selection.gesture_open, `${path}.gesture_open`),
	};
}

export function decodeProgrammingProjection(
	value: unknown,
	path: string,
): ProgrammingProjection {
	const projection = recordAt(value, path);
	return {
		deskId: programmingUuidAt(projection.desk_id, `${path}.desk_id`),
		commandLine: decodeProgrammingCommandLine(
			projection.command_line,
			`${path}.command_line`,
		),
		selection: decodeSelection(projection.selection, `${path}.selection`),
	};
}

export function decodeProgrammingChange(
	value: unknown,
	path: string,
): ProgrammingChange {
	const change = recordAt(value, path);
	const deskId = programmingUuidAt(change.desk_id, `${path}.desk_id`);
	const hasCommandLine = "command_line" in change;
	const hasSelection = "selection" in change;
	if (!hasCommandLine && !hasSelection)
		throw new WireValidationError(
			path,
			"non-empty Programming interaction change",
			change,
		);
	const commandLine = hasCommandLine
		? decodeProgrammingCommandLine(
				change.command_line,
				`${path}.command_line`,
			)
		: null;
	const selection = hasSelection
		? decodeSelection(change.selection, `${path}.selection`)
		: null;
	if (commandLine && selection)
		return { deskId, commandLine, selection };
	if (commandLine) return { deskId, commandLine };
	if (selection) return { deskId, selection };
	throw new WireValidationError(
		path,
		"non-null Programming interaction component",
		change,
	);
}

export function programmingComponentPresence(
	change: ProgrammingChange,
) {
	return {
		commandLine: "commandLine" in change,
		selection: "selection" in change,
	};
}
