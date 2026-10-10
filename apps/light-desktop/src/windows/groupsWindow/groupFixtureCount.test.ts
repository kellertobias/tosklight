import { describe, expect, it } from "vitest";
import { groupFixtureCountLabel, groupFixtureSelection } from "./groupFixtureCount";
import type { Group } from "./model";

const group = (fixtures: string[]): Group => ({
	kind: "group", id: "1", revision: 1, updated_at: "",
	body: { name: "Front", fixtures },
});
const selected = (...fixtures: string[]) => new Set(fixtures);

describe("Group tile fixture membership", () => {
	it("shows total when none of the group members are selected, even if its tile is selected", () => {
		const state = groupFixtureSelection(group(["b", "a"]), selected("elsewhere"), selected("1"));
		expect(state.selected).toBe(true);
		expect(groupFixtureCountLabel(2, state.selectedFixtureCount)).toBe("2");
	});
	it("shows partial and complete fixture intersections without requiring group selection", () => {
		const members = ["b", "unpatched-a"];
		const partial = groupFixtureSelection(group(members), selected("b", "elsewhere"), selected());
		expect(partial.partiallySelected).toBe(true);
		expect(groupFixtureCountLabel(2, partial.selectedFixtureCount)).toBe("1/2");
		const full = groupFixtureSelection(group(members), selected(...members), selected());
		expect(full.fullySelected).toBe(true);
		expect(full.selected).toBe(false);
		expect(groupFixtureCountLabel(2, full.selectedFixtureCount)).toBe("2/2");
		expect(members).toEqual(["b", "unpatched-a"]);
	});
	it("keeps stored empty group selection distinct from an absent slot", () => {
		const empty = groupFixtureSelection(group([]), selected("b"), selected("1"));
		const absent = groupFixtureSelection(null, selected("b"), selected("1"));
		expect(empty.selected).toBe(true);
		expect(absent.selected).toBe(false);
		expect(empty.fullySelected).toBe(false);
		expect(groupFixtureCountLabel(0, empty.selectedFixtureCount)).toBe("0");
	});
});
