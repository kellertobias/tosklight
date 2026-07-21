export type CommandTarget = "FIXTURE" | "GROUP";

export interface CommandChoiceOption {
	id: "plain" | "status";
	label: string;
	command: string;
}

export interface PendingCommandChoice {
	type: "cue_move_copy";
	choiceId: string;
	showId: string;
	showRevision: number;
	operation: "copy" | "move";
	command: string;
	options: readonly CommandChoiceOption[];
	cancelLabel: string;
}

export interface CommandLineProjection {
	text: string;
	target: CommandTarget;
	pristine: boolean;
	revision: number;
	pendingChoice: PendingCommandChoice | null;
}

export type SelectionRule =
	| { type: "all" | "odd" | "even" }
	| { type: "every_nth"; n: number; offset: number };

export type SelectionReference =
	| { type: "fixture" | "remove_fixture"; fixtureId: string }
	| { type: "live_group" | "remove_live_group"; groupId: string };

export type SelectionExpression =
	| { type: "static" }
	| { type: "live_group"; groupId: string; rule: SelectionRule }
	| { type: "frozen_group"; groupId: string; sourceRevision: number }
	| {
			type: "playback_contents" | "sources";
			items: readonly SelectionReference[];
	  };

export interface SelectionProjection {
	selected: readonly string[];
	expression: SelectionExpression | null;
	revision: number;
	gestureOpen: boolean;
}

export type SelectionGestureSource =
	| { type: "fixture"; fixtureId: string }
	| { type: "live_group" | "dereferenced_group"; groupId: string };

export type SelectionAction =
	| {
			type: "replace";
			fixtures: readonly string[];
			expectedRevision: number;
	  }
	| {
			type: "gesture";
			source: SelectionGestureSource;
			remove: boolean;
	  }
	| {
			type: "select_group";
			groupId: string;
			frozen: boolean;
			rule: SelectionRule;
			expectedRevision: number;
	  }
	| { type: "apply_rule"; rule: SelectionRule };

export interface SelectionActionRequest {
	requestId: string;
	action: SelectionAction;
}

export interface SelectionActionOutcome {
	requestId: string;
	correlationId: string;
	action: "replaced" | "gesture_applied" | "group_selected" | "rule_applied";
	applied: number;
	selection: SelectionProjection;
	eventSequence: number;
	replayed: boolean;
	warning: string | null;
}

export interface ProgrammingProjection {
	deskId: string;
	commandLine: CommandLineProjection;
	selection: SelectionProjection;
}

interface ProgrammingChangeBase {
	deskId: string;
}

export type ProgrammingChange =
	| (ProgrammingChangeBase & {
			commandLine: CommandLineProjection;
			selection: SelectionProjection;
	  })
	| (ProgrammingChangeBase & { commandLine: CommandLineProjection })
	| (ProgrammingChangeBase & { selection: SelectionProjection });

export interface ProgrammingSnapshot {
	cursor: number;
	projection: ProgrammingProjection;
}

export type ProgrammingCapability = "commandLine" | "selection";

export type CommandLinePatch = Partial<
	Pick<CommandLineProjection, "text" | "target" | "pristine" | "pendingChoice">
>;

export interface SelectionPatch {
	selected: readonly string[];
	expression?: SelectionExpression | null;
}

export type ProgrammingInteractionEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: ProgrammingChange;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

/** Returns the one live Group represented by the authoritative expression. */
export function selectedGroupId(
	selection: SelectionProjection | null,
): string | null {
	const expression = selection?.expression;
	if (expression?.type === "live_group") return expression.groupId;
	if (expression?.type !== "sources" || expression.items.length !== 1)
		return null;
	const only = expression.items[0];
	return only.type === "live_group" ? only.groupId : null;
}
