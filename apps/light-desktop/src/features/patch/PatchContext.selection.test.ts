import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import {
	reconcileSelectedPatchInstance,
	type SelectedPatchInstance,
} from "./PatchContext";

const fixtures = [
	{
		fixture_id: "fixture-1",
		multipatch: [{ id: "copy-1" }],
	},
] as PatchedFixture[];

describe("patch-local physical selection reconciliation", () => {
	it.each([
		{ fixtureId: "fixture-1", multipatchInstanceId: null },
		{ fixtureId: "fixture-1", multipatchInstanceId: "copy-1" },
	] satisfies SelectedPatchInstance[])("retains an existing physical instance: $multipatchInstanceId", (selection) => {
		expect(reconcileSelectedPatchInstance(selection, fixtures)).toBe(selection);
	});

	it.each([
		{ fixtureId: "removed-fixture", multipatchInstanceId: null },
		{ fixtureId: "fixture-1", multipatchInstanceId: "removed-copy" },
	] satisfies SelectedPatchInstance[])("clears a stale physical instance without parent fallback: $multipatchInstanceId", (selection) => {
		expect(reconcileSelectedPatchInstance(selection, fixtures)).toBeNull();
	});
});
