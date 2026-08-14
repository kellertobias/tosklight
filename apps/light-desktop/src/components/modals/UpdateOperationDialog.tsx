import type {
	CueUpdateMode,
	ExistingContentMode,
	UpdateMode,
	UpdatePreview,
} from "../../api/types";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import {
	cueUpdateModes,
	existingContentModes,
	modeLabel,
	targetFamilyLabel,
} from "../control/updateWorkflow";
import {
	updateAddressLabel,
	updateOutcomeLabel,
	updatePreviewStats,
	updateTargetContext,
} from "./updateWorkflowPresentation";

interface UpdateOperationDialogProps {
	operation: { preview: UpdatePreview; request?: unknown };
	busy: boolean;
	error: string | null;
	onMode: (mode: UpdateMode) => void;
	onApply: () => void;
	onCancel: () => void;
}

export function UpdateOperationDialog({
	operation,
	busy,
	error,
	onMode,
	onApply,
	onCancel,
}: UpdateOperationDialogProps) {
	const { preview } = operation;
	const stats = updatePreviewStats(preview);
	const cueModes = preview.target.family.type === "cue";
	return (
		<ModalRegistration onClose={onCancel}>
			<div
				className="modal-backdrop update-workflow-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onCancel()
				}
			>
			<section
				className="modal-card update-operation-modal workflow-theme update-workflow"
				role="dialog"
				aria-modal="true"
				aria-label={`Update ${preview.target.name}`}
			>
				<ModalTitleBar
					title={<><span>UPDATE</span> {preview.target.name}</>}
					details={updateTargetContext(preview.target)}
					closeLabel="Cancel Update"
					onClose={onCancel}
				/>
				<p>
					Choose how the current programmer changes apply to this existing
					target. Nothing changes until Update is confirmed.
				</p>
				<section className="update-mode-grid" aria-label="Update mode">
					{(cueModes ? cueUpdateModes : existingContentModes).map(
						(candidate) => {
							const mode = cueModes
								? {
										target_type: "cue" as const,
										mode: candidate.value as CueUpdateMode,
									}
								: {
										target_type: "existing_content" as const,
										mode: candidate.value as ExistingContentMode,
									};
							const active =
								preview.mode.target_type === mode.target_type &&
								preview.mode.mode === mode.mode;
							return (
								<Button
									className={active ? "active" : ""}
									disabled={busy}
									onClick={() => onMode(mode)}
									key={candidate.value}
								>
									{candidate.label}
								</Button>
							);
						},
					)}
				</section>
				<div
					className="update-preview-summary"
					role="status"
					aria-label="Update preview summary"
				>
					<strong>{modeLabel(preview.mode)}</strong>
					<span>Eligible {stats.eligible}</span>
					<span>Changed {stats.changed}</span>
					<span>Ignored {stats.ignored}</span>
					{stats.source > 0 && <span>At source {stats.source}</span>}
					{stats.currentCue > 0 && (
						<span>In current Cue {stats.currentCue}</span>
					)}
					{stats.added > 0 && <span>Added {stats.added}</span>}
				</div>
				<section
					className="update-preview-items"
					aria-label="Eligible and ignored programmer changes"
				>
					{preview.items.length === 0 ? (
						<p className="update-no-op">
							The programmer contains no applicable content for this target.
						</p>
					) : (
						preview.items.map((item, index) => (
							<div
								className={`update-preview-item outcome-${item.outcome.outcome}`}
								key={`${updateAddressLabel(item)}-${index}`}
							>
								<b>{updateAddressLabel(item)}</b>
								<span>{updateOutcomeLabel(item)}</span>
							</div>
						))
					)}
				</section>
				{stats.changed === 0 && (
					<p className="update-no-op" role="status">
						No show data would change in this mode.
					</p>
				)}
				{error && (
					<p className="modal-error" role="alert">
						{error}
					</p>
				)}
				<div className="modal-actions">
					<Button disabled={busy} onClick={onCancel}>
						Cancel
					</Button>
					<Button
						className="primary"
						disabled={busy || stats.changed === 0}
						onClick={onApply}
					>
						{busy ? "Updating…" : `Update ${targetFamilyLabel(preview.target)}`}
					</Button>
				</div>
			</section>
			</div>
		</ModalRegistration>
	);
}
