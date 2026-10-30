import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useState } from "react";
import type {
	ShowHealthFinding,
	ShowHealthSnapshot,
} from "./contracts";
import { useApp } from "../../state/AppContext";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";
import { useShowObjectView } from "../showObjects/ShowObjectsView";
import { useShowObjectCollectionsReady } from "../showObjects/ShowObjectsState";
import { useGroupManagement } from "../groupManagement/GroupManagementProvider";
import { publishObjectEditorRequest } from "../controlSurfaceInteraction/objectEditorRequest";
import { PRESET_FAMILIES } from "../../presetFamilies";
import { healthCounts, useShowHealth } from "./useShowHealth";
import "./showHealth.css";

export function ShowHealthModal() {
	const { state, dispatch } = useApp();
	const showId = useActiveShowId();
	return state.showHealthOpen ? (
		<ShowHealthWindow
			key={showId}
			onClose={() =>
				dispatch({ type: "SET_MODAL", modal: "showHealthOpen", value: false })
			}
		/>
	) : null;
}

function ShowHealthWindow({ onClose }: { onClose: () => void }) {
	const check = useShowHealth();
	const { dispatch } = useApp();
	const groupManagement = useGroupManagement();
	useShowObjectView("group", true);
    const groupsReady = useShowObjectCollectionsReady(["group"]);
	const [busy, setBusy] = useState(false);
	const [status, setStatus] = useState<string | null>(null);
	const [lastFix, setLastFix] = useState<{
		id: string;
		revision: number;
		showRevision: number;
	} | null>(null);
	const fix = async (finding?: ShowHealthFinding) => {
		if (!check.snapshot || !groupManagement || (!finding && !lastFix)) return;
		setBusy(true);
		setStatus(null);
		try {
			const id = finding?.object_id ?? lastFix!.id;
			const outcome = await groupManagement.manage({
				objectId: id,
				expectedObjectRevision: finding?.object_revision ?? lastFix!.revision,
				expectedShowRevision: check.snapshot.show_revision,
				operation: finding
					? { type: "remove_missing_fixtures" }
					: { type: "undo" },
			});
			if (!outcome)
				throw new Error(
					"The Group changed or the operation is unavailable. Run the check again before retrying.",
				);
			setLastFix(
				finding && outcome.status === "changed"
					? {
							id,
							revision: outcome.group.revision,
							showRevision: outcome.showRevision,
						}
					: null,
			);
			setStatus(
				finding
					? `Removed missing fixture references from Group ${id}.`
					: `Undid cleanup of Group ${id}.`,
			);
			check.refresh();
		} catch (reason) {
			setStatus(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};
	const jump = (finding: ShowHealthFinding) => {
		if (!check.snapshot) return;
		const showId = check.snapshot.show_id;
		if (finding.fixture_id) {
			publishObjectEditorRequest({
				kind: "fixture",
				objectId: finding.fixture_id,
				showId,
				field: finding.id.includes(":position-") ? "location" : "address",
				multipatchInstanceId: finding.multipatch_instance_id,
			});
			dispatch({ type: "OPEN_BUILTIN", kind: "patch", patchView: "fixtures" });
		} else if (finding.object_kind === "group") {
			publishObjectEditorRequest({
				kind: "group",
				objectId: finding.object_id,
				showId,
			});
			dispatch({ type: "OPEN_BUILTIN", kind: "groups" });
		} else if (finding.object_kind === "cue_list") {
			publishObjectEditorRequest({
				kind: "cuelist",
				objectId: finding.object_id,
				showId,
			});
			dispatch({ type: "OPEN_BUILTIN", kind: "cuelists" });
		} else {
			const family =
				PRESET_FAMILIES[Number(finding.object_id.split(".")[0])] ?? "Mixed";
			dispatch({ type: "SET_PRESET_FAMILY", family });
			publishObjectEditorRequest({
				kind: "preset",
				objectId: finding.object_id,
				showId,
			});
			dispatch({ type: "OPEN_BUILTIN", kind: "presets" });
		}
		onClose();
	};
	return (
		<ModalPortal onClose={onClose}>
			<div className="modal-backdrop">
				<section
					className="modal-card show-health-window"
					role="dialog"
					aria-modal="true"
					aria-label="Show Health"
				>
					<ModalTitleBar title="Show Health" onClose={onClose} />
					<div className="show-health-actions">
						<Button
							disabled={check.loading || busy || !check.showId}
							onClick={check.refresh}
						>
							Run check
						</Button>
						{lastFix && (
							<Button disabled={busy || !groupsReady || check.loading || !check.snapshot} onClick={() => void fix()}>
								Undo last cleanup
							</Button>
						)}
					</div>
					{check.loading && <p role="status">Checking the active show…</p>}
					{!check.showId && (
						<p role="status">Open a show to run Show Health.</p>
					)}
					{check.error && (
						<p role="alert">
							Cannot check show: {check.error}. Check the connection and run
							again.
						</p>
					)}
					{status && <p role="status">{status}</p>}
					{check.snapshot && (
						<HealthFindings
							snapshot={check.snapshot}
							busy={busy}
                            fixesReady={groupsReady}
							jump={jump}
							fix={(finding) => void fix(finding)}
						/>
					)}
				</section>
			</div>
		</ModalPortal>
	);
}

function HealthFindings({
	snapshot,
	busy,
    fixesReady,
	jump,
	fix,
}: {
	snapshot: ShowHealthSnapshot;
	busy: boolean;
    fixesReady: boolean;
	jump: (finding: ShowHealthFinding) => void;
	fix: (finding: ShowHealthFinding) => void;
}) {
	return (
		<>
			<p>
				Checked {new Date(snapshot.checked_at).toLocaleTimeString()} ·{" "}
				{healthCounts(snapshot)}
			</p>
			<p>
				Checks: {snapshot.checks.join("; ")}. Findings do not change live
				output.
			</p>
			{(["error", "warning", "info"] as const).map((severity) => (
				<section key={severity} aria-label={`${severity} findings`}>
					<h3>
						{severity === "info"
							? "Information"
							: severity === "error"
								? "Errors"
								: "Warnings"}
					</h3>
					{snapshot.findings.filter((finding) => finding.severity === severity)
						.length === 0 && <p>None in these checks.</p>}
					<ul>
						{snapshot.findings
							.filter((finding) => finding.severity === severity)
							.map((finding) => (
								<li key={finding.id} data-severity={severity}>
									<p>{finding.message}</p>
									<div className="show-health-actions">
										<Button disabled={busy} onClick={() => jump(finding)}>
											Go to{" "}
											{finding.object_kind === "patched_fixture"
												? "fixture"
												: finding.object_kind.replaceAll("_", " ")}
										</Button>
										{finding.remove_missing_group_fixtures && (
											<Button disabled={busy || !fixesReady} onClick={() => fix(finding)}>
												Remove missing fixtures
											</Button>
										)}
									</div>
								</li>
							))}
					</ul>
				</section>
			))}
		</>
	);
}
