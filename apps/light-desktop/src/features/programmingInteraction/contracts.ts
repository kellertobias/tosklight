export type CommandTarget = "FIXTURE" | "GROUP";

export interface CommandChoiceOption {
	id: "plain" | "status";
	label: string;
	command: string;
}

export interface CueMoveCopyChoice {
	type: "cue_move_copy";
	choiceId: string;
	showId: string;
	showRevision: number;
	operation: "copy" | "move";
	command: string;
	options: readonly CommandChoiceOption[];
	cancelLabel: string;
}

export interface DynamicInstanceChoiceOption {
	controllerId: string;
	label: string;
	command: string;
}

export interface DynamicInstanceChoice {
	type: "dynamic_instance";
	choiceId: string;
	showId: string;
	showRevision: number;
	dynamicId: string;
	poolNumber: number;
	command: string;
	options: readonly DynamicInstanceChoiceOption[];
	cancelLabel: string;
}

export type PendingCommandChoice =
	| CueMoveCopyChoice
	| DynamicInstanceChoice;

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
	| {
			type: "playback_contents" | "sources";
			items: readonly SelectionReference[];
	  };

export type SelectionGridMethod =
	| "stage2d"
	| "top_to_bottom"
	| "bottom_to_top"
	| "front_to_back"
	| "back_to_front"
	| "left_to_right"
	| "right_to_left"
	| "horizontal_axis_x"
	| "vertical_axis_z"
	| "room_depth_axis_y";

export interface SelectionGridConfiguration {
	method: SelectionGridMethod;
	axisOrigin: { x: number; y: number; z: number };
}

export interface SelectionGridState {
	configuration: SelectionGridConfiguration;
	rowsFirst: "top_left" | "top_right" | "bottom_left" | "bottom_right";
	columnsFirst: "top_left" | "bottom_left" | "top_right" | "bottom_right";
}

export const DEFAULT_SELECTION_GRID_STATE: SelectionGridState = {
	configuration: {
		method: "stage2d",
		axisOrigin: { x: 0, y: 0, z: 0 },
	},
	rowsFirst: "top_left",
	columnsFirst: "top_left",
};

export interface SelectionProjection {
	selected: readonly string[];
	expression: SelectionExpression | null;
	revision: number;
	gestureOpen: boolean;
	grid: SelectionGridState;
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
	| { type: "apply_rule"; rule: SelectionRule }
	| { type: "cycle_grid_method" }
	| {
			type: "set_grid_configuration";
			configuration: SelectionGridConfiguration;
			expectedRevision: number;
	  }
	| { type: "reorder_from_grid"; axis: "rows" | "columns" };

export interface SelectionActionRequest {
	requestId: string;
	action: SelectionAction;
}

export interface SelectionActionOutcome {
	requestId: string;
	correlationId: string;
	action:
		| "replaced"
		| "gesture_applied"
		| "group_selected"
		| "rule_applied"
		| "grid_method_cycled"
		| "grid_configuration_set"
		| "grid_reordered";
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
