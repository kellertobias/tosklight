import { describe, expect, it } from "vitest";
import { groupReferencePresentation } from "./GroupCard";

describe("groupReferencePresentation", () => {
	it("uses compact referenced Group numbers and retains the full wording", () => {
		expect(groupReferencePresentation([{ group_id: "27" }])).toEqual({
			compact: "Ref: 27",
			description: "References Group 27",
		});
		expect(
			groupReferencePresentation([{ group_id: "4" }, { group_id: "9" }]),
		).toEqual({
			compact: "Ref: 4, 9",
			description: "References Groups 4, 9",
		});
	});
});
