import type {
	CommandLineResponse,
	ProgrammerSelectionProjection,
	ProgrammingInteractionChange,
} from "./generated/light-wire";

export const PROGRAMMING_DESK_ID =
	"11111111-1111-4111-8111-111111111111";
export const OTHER_PROGRAMMING_DESK_ID =
	"99999999-9999-4999-8999-999999999999";
export const PROGRAMMING_FIXTURE_ID =
	"22222222-2222-4222-8222-222222222222";

export function programmingCommandLine(
	overrides: Partial<CommandLineResponse> = {},
): CommandLineResponse {
	return {
		text: "FIXTURE 1",
		target: "FIXTURE",
		pristine: false,
		revision: 4,
		pending_choice: null,
		...overrides,
	};
}

export function programmingSelection(
	overrides: Partial<ProgrammerSelectionProjection> = {},
): ProgrammerSelectionProjection {
	return {
		selected: [PROGRAMMING_FIXTURE_ID],
		expression: {
			type: "live_group",
			group_id: "7",
			rule: { type: "every_nth", n: 2, offset: 0 },
		},
		revision: 6,
		...overrides,
	};
}

export function programmingSnapshot() {
	return {
		cursor: { sequence: 20 },
		projection: {
			desk_id: PROGRAMMING_DESK_ID,
			command_line: programmingCommandLine(),
			selection: programmingSelection(),
		},
	};
}

export function programmingChange(
	capabilities: "commandLine" | "selection" | "both" = "both",
): ProgrammingInteractionChange {
	if (capabilities === "commandLine")
		return {
			desk_id: PROGRAMMING_DESK_ID,
			command_line: programmingCommandLine(),
		};
	if (capabilities === "selection")
		return {
			desk_id: PROGRAMMING_DESK_ID,
			selection: programmingSelection(),
		};
	return {
		desk_id: PROGRAMMING_DESK_ID,
		command_line: programmingCommandLine(),
		selection: programmingSelection(),
	};
}

export function programmingEvent(
	capabilities: "commandLine" | "selection" | "both" = "both",
) {
	const change = programmingChange(capabilities);
	return {
		type: "event",
		event: {
			sequence: 21,
			occurred_at: "2026-07-19T10:00:00Z",
			desk_id: PROGRAMMING_DESK_ID,
			class: "projection",
			object: {
				capability: "desk",
				id:
					capabilities === "selection"
						? `programming-selection:${PROGRAMMING_DESK_ID}`
						: `programming-command-line:${PROGRAMMING_DESK_ID}`,
			},
			related_objects:
				capabilities === "both"
					? [
							{
								capability: "desk",
								id: `programming-selection:${PROGRAMMING_DESK_ID}`,
							},
						]
					: [],
			source: { kind: "action", source: "osc" },
			correlation_id: "33333333-3333-4333-8333-333333333333",
			delivery: "lossless",
			payload: { type: "programming_interaction_changed", change },
		},
	};
}
