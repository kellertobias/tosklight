import { expect, it } from "vitest";
import { decodeShowHealth } from "./showHealthWire";
import { encodeGroupManagementRequest } from "./groupManagementWire";

it("validates health diagnostics and encodes only the narrow undoable Group intent", () => {
	const snapshot = {
		show_id: "show",
		show_revision: 3,
		checked_at: "now",
		checks: [],
		findings: [],
	};
	expect(decodeShowHealth(snapshot)).toEqual(snapshot);
	expect(() =>
		decodeShowHealth({ ...snapshot, findings: [{ severity: "fatal" }] }),
	).toThrow();
	expect(
		encodeGroupManagementRequest({
			requestId: "fix",
			groupId: "7",
			expectedObjectRevision: 2,
			expectedShowRevision: 3,
			operation: { type: "remove_missing_fixtures" },
		}),
	).toEqual({
		request_id: "fix",
		group_id: "7",
		expected_object_revision: 2,
		expected_show_revision: 3,
		operation: { type: "remove_missing_fixtures" },
	});
});
