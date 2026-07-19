import { describe, expect, it } from "vitest";
import type {
	CommandLineProjection,
	ProgrammingChange,
	ProgrammingSnapshot,
	SelectionExpression,
	SelectionProjection,
} from "../features/programmingInteraction/contracts";
import type { ProgrammingEventScope } from "../features/programmingInteraction/transport";
import {
	decodeProgrammingCommandLine,
	decodeProgrammingEventMessage,
	decodeProgrammingInteractionSnapshot,
} from "./programmingWire";
import {
	OTHER_PROGRAMMING_DESK_ID,
	PROGRAMMING_DESK_ID,
	PROGRAMMING_FIXTURE_ID,
	programmingCommandLine,
	programmingEvent,
	programmingSnapshot,
} from "./programmingWireTestFixtures";

const BOTH: ProgrammingEventScope = { commandLine: true, selection: true };
const COMMAND: ProgrammingEventScope = {
	commandLine: true,
	selection: false,
};
const SELECTION: ProgrammingEventScope = {
	commandLine: false,
	selection: true,
};

function record(value: unknown) {
	return value as Record<string, unknown>;
}

function decodedCommandLine(revision = 4): CommandLineProjection {
	return {
		text: "FIXTURE 1",
		target: "FIXTURE",
		pristine: false,
		revision,
		pendingChoice: null,
	};
}

function decodedSelection(
	expression: SelectionExpression = {
		type: "live_group",
		groupId: "7",
		rule: { type: "every_nth", n: 2, offset: 0 },
	},
): SelectionProjection {
	return {
		selected: [PROGRAMMING_FIXTURE_ID],
		expression,
		revision: 6,
	};
}

function decodedSnapshot(): ProgrammingSnapshot {
	return {
		cursor: 20,
		projection: {
			deskId: PROGRAMMING_DESK_ID,
			commandLine: decodedCommandLine(),
			selection: decodedSelection(),
		},
	};
}

function decodedChange(
	capability: "commandLine" | "selection" | "both",
): ProgrammingChange {
	if (capability === "commandLine")
		return { deskId: PROGRAMMING_DESK_ID, commandLine: decodedCommandLine() };
	if (capability === "selection")
		return { deskId: PROGRAMMING_DESK_ID, selection: decodedSelection() };
	return {
		deskId: PROGRAMMING_DESK_ID,
		commandLine: decodedCommandLine(),
		selection: decodedSelection(),
	};
}

describe("Programming projection wire validation", () => {
	it("decodes a complete snapshot and discards untrusted fields", () => {
		const value = programmingSnapshot();
		record(value.projection).untrusted = true;
		record(value.projection.command_line).untrusted = "command";
		record(value.projection.selection).untrusted = "selection";

		const decoded = decodeProgrammingInteractionSnapshot(
			value,
			PROGRAMMING_DESK_ID,
		);

		expect(decoded).toEqual(decodedSnapshot());
		expect("untrusted" in decoded.projection).toBe(false);
		expect("untrusted" in decoded.projection.commandLine).toBe(false);
		expect("untrusted" in decoded.projection.selection).toBe(false);
		expect(decoded.projection.selection.expression).toMatchObject({
			type: "live_group",
			groupId: "7",
			rule: { type: "every_nth", n: 2, offset: 0 },
		});
	});

	it("decodes command choice fields without rejecting an empty command", () => {
		expect(
			decodeProgrammingCommandLine({
				...programmingCommandLine(),
				text: "",
				pending_choice: {
					type: "cue_move_copy",
					operation: "copy",
					command: "CUE 1 COPY CUE 2",
					options: [{ id: "plain", label: "Copy", command: "ENT" }],
					cancel_label: "Cancel",
				},
			}),
		).toMatchObject({ text: "", pendingChoice: { operation: "copy" } });
	});

	it("decodes ordered selection-source variants with operator Group IDs", () => {
		const value = programmingSnapshot();
		value.projection.selection.expression = {
			type: "sources",
			items: [
				{ type: "fixture", fixture_id: PROGRAMMING_FIXTURE_ID },
				{ type: "remove_live_group", group_id: "42" },
			],
		};

		expect(
			decodeProgrammingInteractionSnapshot(value, PROGRAMMING_DESK_ID)
				.projection.selection.expression,
		).toEqual({
			type: "sources",
			items: [
				{ type: "fixture", fixtureId: PROGRAMMING_FIXTURE_ID },
				{ type: "remove_live_group", groupId: "42" },
			],
		});
	});

	it.each([
		["requested desk", (value: ReturnType<typeof programmingSnapshot>) => value],
		["snapshot desk", (value: ReturnType<typeof programmingSnapshot>) => {
			value.projection.desk_id = OTHER_PROGRAMMING_DESK_ID;
			return value;
		}],
		["cursor", (value: ReturnType<typeof programmingSnapshot>) => {
			value.cursor.sequence = -1;
			return value;
		}],
		["command revision", (value: ReturnType<typeof programmingSnapshot>) => {
			value.projection.command_line.revision = 1.5;
			return value;
		}],
		["selection revision", (value: ReturnType<typeof programmingSnapshot>) => {
			value.projection.selection.revision = Number.MAX_SAFE_INTEGER + 1;
			return value;
		}],
		["fixture UUID", (value: ReturnType<typeof programmingSnapshot>) => {
			value.projection.selection.selected = ["fixture-1"];
			return value;
		}],
		["selection expression", (value: ReturnType<typeof programmingSnapshot>) => {
			record(value.projection.selection.expression).type = "live_groups";
			return value;
		}],
		["Every Nth divisor", (value: ReturnType<typeof programmingSnapshot>) => {
			const expression = record(value.projection.selection.expression);
			record(expression.rule).n = 0;
			return value;
		}],
		["selection reference", (value: ReturnType<typeof programmingSnapshot>) => {
			value.projection.selection.expression = {
				type: "sources",
				items: [{ type: "fixture", fixture_id: "fixture-1" }],
			};
			return value;
		}],
	])("rejects an invalid %s", (label, mutate) => {
		const expectedDesk = label === "requested desk" ? "desk-1" : PROGRAMMING_DESK_ID;
		expect(() =>
			decodeProgrammingInteractionSnapshot(
				mutate(programmingSnapshot()),
				expectedDesk,
			),
		).toThrow();
	});
});

describe("Programming event wire validation", () => {
	it.each([
		["commandLine", COMMAND],
		["selection", SELECTION],
		["both", COMMAND],
		["both", SELECTION],
		["both", BOTH],
	] as const)(
		"accepts a %s change routed through a subscribed component",
		(capability, scope) => {
			const value = programmingEvent(capability);
			expect(
				decodeProgrammingEventMessage(value, PROGRAMMING_DESK_ID, scope),
			).toEqual({
				type: "event",
				sequence: 21,
				change: decodedChange(capability),
			});
		},
	);

	it("decodes cursors and ignores another event capability", () => {
		expect(
			decodeProgrammingEventMessage(
				{ type: "ready", cursor: { sequence: 20 } },
				PROGRAMMING_DESK_ID,
				COMMAND,
			),
		).toEqual({ type: "ready", cursor: 20 });
		expect(
			decodeProgrammingEventMessage(
				{
					type: "gap",
					gap: {
						after_sequence: 20,
						oldest_available: 22,
						latest_sequence: 25,
					},
				},
				PROGRAMMING_DESK_ID,
				COMMAND,
			),
		).toEqual({
			type: "gap",
			afterSequence: 20,
			oldestAvailable: 22,
			latestSequence: 25,
		});
		expect(
			decodeProgrammingEventMessage(
				{
					type: "event",
					event: {
						sequence: 26,
						payload: { type: "output_runtime_changed" },
					},
				},
				PROGRAMMING_DESK_ID,
				COMMAND,
			),
		).toBeNull();
	});

	it.each([
		["wrong class", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.class = "transition";
		}],
		["replaceable delivery", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.delivery = "replaceable";
		}],
		["unsafe sequence", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.sequence = Number.MAX_SAFE_INTEGER + 1;
		}],
		["wrong envelope desk", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.desk_id = OTHER_PROGRAMMING_DESK_ID;
		}],
		["wrong payload desk", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.payload.change.desk_id = OTHER_PROGRAMMING_DESK_ID;
		}],
		["null primary route", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			record(event.event).object = null;
		}],
		["legacy route", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.object.id = `programming-interaction:${PROGRAMMING_DESK_ID}`;
		}],
		["extra route", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.related_objects.push({
				capability: "desk",
				id: `programming-selection:${PROGRAMMING_DESK_ID}`,
			});
		}],
		["duplicate route", "both", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.related_objects.push(event.event.related_objects[0]);
		}],
		["reversed combined routes", "both", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.object.id = `programming-selection:${PROGRAMMING_DESK_ID}`;
			event.event.related_objects[0].id = `programming-command-line:${PROGRAMMING_DESK_ID}`;
		}],
		["null component", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			record(event.event.payload.change).command_line = null;
		}],
		["invalid correlation UUID", "commandLine", BOTH, (event: ReturnType<typeof programmingEvent>) => {
			event.event.correlation_id = "request-1";
		}],
		["unsubscribed component", "commandLine", SELECTION, () => {}],
	] as const)("rejects %s", (_label, capability, scope, mutate) => {
		const event = programmingEvent(capability);
		mutate(event);
		expect(() =>
			decodeProgrammingEventMessage(event, PROGRAMMING_DESK_ID, scope),
		).toThrow();
	});
});
