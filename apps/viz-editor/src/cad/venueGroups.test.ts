import { describe, expect, it } from "vitest";
import type { CadEntity } from "./types";
import {
	expandToGroups,
	groupOf,
	groupSelection,
	nextGroupName,
	ungroupSelection,
	venueGroupAction,
	type VenueGroups,
} from "./venueGroups";

const entity = (id: string, kind = "venue") =>
	({ id, logicalFixtureId: id, kind }) as CadEntity;

const rig: VenueGroups = {
	groups: [{ id: "g1", name: "Group 1", memberIds: ["a", "b"] }],
};

describe("Venue element groups", () => {
	it("widens a pick to its whole group and keeps an ungrouped element alone", () => {
		expect(expandToGroups(rig, ["b"])).toEqual(["a", "b"]);
		expect(expandToGroups(rig, ["c", "a", "b"])).toEqual(["c", "a", "b"]);
		expect(expandToGroups(rig, [])).toEqual([]);
		expect(groupOf(rig, "c")).toBeUndefined();
	});

	it("groups the selection, taking elements out of the group they were in", () => {
		const next = groupSelection(rig, ["b", "c"], "Upstage", "g2");
		expect(next.groups).toEqual([
			{ id: "g1", name: "Group 1", memberIds: ["a"] },
			{ id: "g2", name: "Upstage", memberIds: ["b", "c"] },
		]);
		expect(groupSelection(rig, ["a", "b", "c"], "All", "g3").groups).toEqual([
			{ id: "g3", name: "All", memberIds: ["a", "b", "c"] },
		]);
		expect(nextGroupName(rig)).toBe("Group 2");
	});

	it("ungroups every group a selected element is in and leaves the others", () => {
		const two: VenueGroups = {
			groups: [...rig.groups, { id: "g2", name: "Group 2", memberIds: ["c", "d"] }],
		};
		expect(ungroupSelection(two, ["a"]).groups.map((group) => group.id)).toEqual(["g2"]);
	});

	it("offers Group for two Venue elements and Ungroup only for a grouped selection", () => {
		const entities = [entity("a"), entity("b"), entity("c"), entity("lamp", "profile")];
		expect(venueGroupAction(rig, entities, ["c", "lamp"], "group")).toBeNull();
		expect(venueGroupAction(rig, entities, ["a", "b"], "group")).toBeNull();
		expect(
			venueGroupAction(rig, entities, ["a", "b", "c", "lamp"], "group")?.groups[0].memberIds,
		).toEqual(["a", "b", "c"]);
		expect(venueGroupAction(rig, entities, ["c"], "ungroup")).toBeNull();
		expect(venueGroupAction(rig, entities, ["a", "b"], "ungroup")).toEqual({ groups: [] });
	});
});
