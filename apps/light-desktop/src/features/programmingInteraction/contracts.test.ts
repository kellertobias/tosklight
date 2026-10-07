import { describe, expect, it } from "vitest";
import { selectedGroupIds, type SelectionExpression } from "./contracts";

const groups = (expression: SelectionExpression | null) => selectedGroupIds({
	selected: ["fixture-1"], expression, revision: 1, gestureOpen: false,
});

describe("selected live Group references", () => {
	it("does not infer a Group target from selected fixtures", () => {
		expect(groups(null)).toEqual([]);
		expect(groups({ type: "static" })).toEqual([]);
	});
	it("retains a live Group target even with a membership rule", () => {
		expect(groups({ type: "live_group", groupId: "1", rule: { type: "odd" } })).toEqual(["1"]);
	});
	it("tracks multiple references and explicit removals in source order", () => {
		expect(groups({ type: "sources", items: [
			{ type: "live_group", groupId: "1" },
			{ type: "live_group", groupId: "2" },
			{ type: "remove_live_group", groupId: "1" },
			{ type: "fixture", fixtureId: "fixture-1" },
			{ type: "live_group", groupId: "1" },
		] })).toEqual(["2", "1"]);
	});
});
