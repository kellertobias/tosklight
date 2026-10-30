import type { ShowHealthSnapshot } from "../features/showHealth/contracts";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";

export function decodeShowHealth(value: unknown): ShowHealthSnapshot {
	const root = recordAt(value, "showHealth");
	return {
		show_id: stringAt(root.show_id, "show_id"),
		show_revision: integerAt(root.show_revision, "show_revision"),
		checked_at: stringAt(root.checked_at, "checked_at"),
		checks: arrayAt(root.checks, "checks").map((value) =>
			stringAt(value, "check"),
		),
		findings: arrayAt(root.findings, "findings").map((value) => {
			const row = recordAt(value, "finding");
			return {
				id: stringAt(row.id, "id"),
				severity: enumAt(row.severity, "severity", [
					"error",
					"warning",
					"info",
				] as const),
				message: stringAt(row.message, "message"),
				object_kind: stringAt(row.object_kind, "object_kind"),
				object_id: stringAt(row.object_id, "object_id"),
				object_revision: integerAt(row.object_revision, "object_revision"),
				fixture_id:
					row.fixture_id == null
						? null
						: stringAt(row.fixture_id, "fixture_id"),
				multipatch_instance_id:
					row.multipatch_instance_id == null
						? null
						: stringAt(row.multipatch_instance_id, "multipatch_instance_id"),
				remove_missing_group_fixtures: booleanAt(
					row.remove_missing_group_fixtures,
					"remove_missing_group_fixtures",
				),
			};
		}),
	};
}
