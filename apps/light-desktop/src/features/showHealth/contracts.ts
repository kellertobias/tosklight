/** Validated active-show diagnostics consumed by the Show Health window. */
export interface ShowHealthFinding {
	id: string;
	severity: "error" | "warning" | "info";
	message: string;
	object_kind: string;
	object_id: string;
	object_revision: number;
	fixture_id: string | null;
	multipatch_instance_id: string | null;
	remove_missing_group_fixtures: boolean;
}

export interface ShowHealthSnapshot {
	show_id: string;
	show_revision: number;
	checked_at: string;
	checks: string[];
	findings: ShowHealthFinding[];
}
