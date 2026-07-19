import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { StoredGroup, VersionedObject } from "../../api/types";
import { useSelectedGroupMembership } from "./useSelectedGroupMembership";

function group(master: number, fixtures: string[]): VersionedObject<StoredGroup> {
	return {
		kind: "group",
		id: "1",
		revision: 1,
		updated_at: "",
		body: { name: "Front", fixtures, master },
	};
}

describe("useSelectedGroupMembership", () => {
	it("does not reapply unchanged membership for runtime-only Group samples", () => {
		const fixtures = ["fixture-1", "fixture-2"];
		const setSelectedGroupId = vi.fn();
		const setSelectedFixtures = vi.fn();
		const { rerender } = renderHook(
			({ groups }) =>
				useSelectedGroupMembership(
					groups,
					"1",
					setSelectedGroupId,
					setSelectedFixtures,
				),
			{ initialProps: { groups: [group(0.2, fixtures)] } },
		);
		expect(setSelectedFixtures).toHaveBeenCalledOnce();

		rerender({ groups: [group(0.7, fixtures)] });

		expect(setSelectedFixtures).toHaveBeenCalledOnce();
		expect(setSelectedGroupId).not.toHaveBeenCalled();
	});
});
