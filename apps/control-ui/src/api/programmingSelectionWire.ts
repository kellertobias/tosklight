import type {
	SelectionAction,
	SelectionActionOutcome,
	SelectionActionRequest,
	SelectionGestureSource,
	SelectionRule,
} from "../features/programmingInteraction/contracts";
import type {
	ProgrammerSelectionRule,
	ProgrammingSelectionAction,
	ProgrammingSelectionActionRequest,
	ProgrammingSelectionGestureSource,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammingSelection,
	programmingUuidAt,
} from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

export function encodeSelectionActionRequest(
	request: SelectionActionRequest,
): ProgrammingSelectionActionRequest {
	return {
		request_id: request.requestId,
		...encodeAction(request.action),
	};
}

export function decodeSelectionActionOutcome(
	value: unknown,
	expectedRequestId: string,
): SelectionActionOutcome {
	const outcome = recordAt(value, "$");
	const requestId = stringAt(outcome.request_id, "$.request_id");
	if (requestId !== expectedRequestId)
		throw new WireValidationError(
			"$.request_id",
			`request ID ${expectedRequestId}`,
			requestId,
		);
	return {
		requestId,
		correlationId: programmingUuidAt(
			outcome.correlation_id,
			"$.correlation_id",
		),
		action: enumAt(outcome.action, "$.action", [
			"replaced",
			"gesture_applied",
			"group_selected",
			"rule_applied",
		]),
		applied: integerAt(outcome.applied, "$.applied"),
		selection: decodeProgrammingSelection(outcome.selection, "$.selection"),
		eventSequence: integerAt(outcome.event_sequence, "$.event_sequence"),
		replayed: booleanAt(outcome.replayed, "$.replayed"),
		warning:
			outcome.warning == null
				? null
				: stringAt(outcome.warning, "$.warning"),
	};
}

function encodeAction(action: SelectionAction): ProgrammingSelectionAction {
	switch (action.type) {
		case "replace":
			return {
				action: "replace",
				fixtures: [...action.fixtures],
				expected_revision: action.expectedRevision,
			};
		case "gesture":
			return {
				action: "gesture",
				source: encodeSource(action.source),
				remove: action.remove,
			};
		case "select_group":
			return {
				action: "select_group",
				group_id: action.groupId,
				frozen: action.frozen,
				rule: encodeRule(action.rule),
				expected_revision: action.expectedRevision,
			};
		case "apply_rule":
			return { action: "apply_rule", rule: encodeRule(action.rule) };
	}
}

function encodeSource(
	source: SelectionGestureSource,
): ProgrammingSelectionGestureSource {
	if (source.type === "fixture")
		return { type: source.type, fixture_id: source.fixtureId };
	return { type: source.type, group_id: source.groupId };
}

function encodeRule(rule: SelectionRule): ProgrammerSelectionRule {
	return rule.type === "every_nth"
		? { type: rule.type, n: rule.n, offset: rule.offset }
		: { type: rule.type };
}
