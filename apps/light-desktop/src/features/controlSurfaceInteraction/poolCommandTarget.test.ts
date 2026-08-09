import { describe, expect, it } from "vitest";
import {
	canonicalPoolMutationOperation,
	poolMutationTarget,
	poolMutationTargetState,
} from "./poolCommandTarget";

describe("pool command targeting", () => {
	it.each([
		["COPY", { operation: "copy", phase: "source" }],
		["CPY", { operation: "copy", phase: "source" }],
		["MOVE 2.4 AT", { operation: "move", phase: "destination", source: "2.4" }],
		["DEL", { operation: "delete", phase: "source" }],
	])("recognizes an exact actionable phase for %s", (command, expected) => {
		expect(poolMutationTarget(command)).toEqual(expected);
	});

	it.each(["", "COPY 2", "COPY 2.4", "COPY 2.4 AT 6", "DELETE 2.4"])(
		"leaves incomplete and complete addresses to ordinary command entry: %s",
		(command) => expect(poolMutationTarget(command)).toBeNull(),
	);

	it("maps actions to literal non-color-only pool states", () => {
		expect(poolMutationTargetState(poolMutationTarget("MOVE"))).toBe(
			"move-target",
		);
		expect(canonicalPoolMutationOperation("copy")).toBe("COPY");
	});
});
