import { describe, expect, it } from "vitest";
import type { ProgrammingChange } from "./contracts";
import { ProgrammingViewScope } from "./scope";

const DESK_ID = "11111111-1111-4111-8111-111111111111";

function commandChange(): ProgrammingChange {
	return {
		deskId: DESK_ID,
		commandLine: {
			text: "FIXTURE",
			target: "FIXTURE",
			pristine: true,
			revision: 1,
			pendingChoice: null,
		},
	};
}

function selectionChange(): ProgrammingChange {
	return {
		deskId: DESK_ID,
		selection: { selected: [], expression: null, revision: 1 },
	};
}

describe("ProgrammingViewScope", () => {
	it("reference-counts command-line and selection views independently", () => {
		const scope = new ProgrammingViewScope();
		scope.activate("commandLine");
		scope.activate("commandLine");
		scope.activate("selection");

		expect(scope.subscription()).toEqual({
			commandLine: true,
			selection: true,
		});
		expect(scope.deactivate("commandLine")).toBe(false);
		expect(scope.subscription().commandLine).toBe(true);
		expect(scope.deactivate("commandLine")).toBe(true);
		expect(scope.subscription()).toEqual({
			commandLine: false,
			selection: true,
		});
	});

	it("reports only changes relevant to mounted capabilities", () => {
		const scope = new ProgrammingViewScope();
		scope.activate("selection");
		const selected = selectionChange();
		if (!("selection" in selected)) throw new Error("invalid test fixture");

		expect(scope.includesChange(commandChange())).toBe(false);
		expect(scope.includesChange(selected)).toBe(true);
		expect(
			scope.includesChange({
				...commandChange(),
				selection: selected.selection,
			}),
		).toBe(true);
	});

	it("has a stable key and clears all references", () => {
		const scope = new ProgrammingViewScope();
		expect(scope.hasViews()).toBe(false);
		scope.activate("selection");
		expect(scope.key()).toBe(
			JSON.stringify({ commandLine: false, selection: true }),
		);
		scope.clear();
		expect(scope.hasViews()).toBe(false);
		expect(scope.subscription()).toEqual({
			commandLine: false,
			selection: false,
		});
	});
});
