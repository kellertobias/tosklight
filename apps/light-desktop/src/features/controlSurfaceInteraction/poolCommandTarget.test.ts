import { describe, expect, it } from "vitest";
import {
	canonicalPoolMutationOperation,
	poolMutationTarget,
	poolMutationTargetState,
	poolObjectMutationCommand,
} from "./poolCommandTarget";

describe("pool command targeting", () => {
	it.each([
		["COPY", { operation: "copy", phase: "source" }],
		["CPY", { operation: "copy", phase: "source" }],
		[
			"MOVE COLOR PRESET 4 AT",
			{
				operation: "move",
				phase: "destination",
				source: "COLOR PRESET 4",
			},
		],
		["DEL", { operation: "delete", phase: "source" }],
	])("recognizes an exact actionable phase for %s", (command, expected) => {
		expect(poolMutationTarget(command)).toEqual(expected);
	});

	it.each([
		"",
		"COPY 2",
		"COPY 2.4",
		"COPY COLOR PRESET 4 AT 6",
		"DELETE 2.4",
	])("leaves incomplete and complete addresses to ordinary command entry: %s", (command) =>
		expect(poolMutationTarget(command)).toBeNull());

	it("maps actions to literal non-color-only pool states", () => {
		expect(poolMutationTargetState(poolMutationTarget("MOVE"))).toBe(
			"move-target",
		);
		expect(canonicalPoolMutationOperation("copy")).toBe("COPY");
	});

	it("builds compatible typed object commands for touch on either side", () => {
		expect(
			poolObjectMutationCommand(poolMutationTarget("COPY"), "GROUP", 4, true),
		).toEqual({
			kind: "replace",
			command: "COPY GROUP 4 AT",
		});
		expect(
			poolObjectMutationCommand(
				poolMutationTarget("COPY GROUP 4 AT"),
				"CUELIST",
				7,
				false,
			),
		).toBeNull();
		expect(
			poolObjectMutationCommand(
				poolMutationTarget("MOVE CUELIST 1 AT"),
				"CUELIST",
				2,
				false,
			),
		).toEqual({
			kind: "execute",
			command: "MOVE CUELIST 1 AT CUELIST 2",
		});
	});
});
