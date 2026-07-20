import type {
	UpdateMenuEntry,
	UpdateMode,
	UpdatePreview,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
} from "../../api/types";
import { Button, SelectField } from "../common";
import {
	modeLabel,
	targetFamilyLabel,
	updateTargetKey,
} from "../control/updateWorkflow";
import { UpdateDefaultsFields } from "../setup/ProgrammerDefaults";
import {
	updatePreviewStats,
	updateTargetContext,
} from "./updateWorkflowPresentation";

interface UpdateSettingsDialogProps {
	settings: UpdateSettings;
	busy: boolean;
	error: string | null;
	onChange: (settings: UpdateSettings) => void;
	onSave: () => void;
	onCancel: () => void;
}

export function UpdateSettingsDialog({
	settings,
	busy,
	error,
	onChange,
	onSave,
	onCancel,
}: UpdateSettingsDialogProps) {
	return (
		<div
			className="modal-backdrop update-workflow-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onCancel()
			}
		>
			<section
				className="modal-card update-settings-modal workflow-theme update-workflow"
				role="dialog"
				aria-modal="true"
				aria-label="Update Settings"
			>
				<Button
					className="modal-close"
					aria-label="Close Update Settings"
					onClick={onCancel}
				>
					×
				</Button>
				<h2>
					<span className="workflow-badge">UPDATE</span> Update Settings
				</h2>
				<p>
					Desk workflow preferences for Update. These settings do not change
					show programming.
				</p>
				<UpdateDefaultsFields settings={settings} onChange={onChange} />
				{error && (
					<p className="modal-error" role="alert">
						{error}
					</p>
				)}
				<div className="modal-actions">
					<Button disabled={busy} onClick={onCancel}>
						Cancel
					</Button>
					<Button className="primary" disabled={busy} onClick={onSave}>
						{busy ? "Saving…" : "Save Update Settings"}
					</Button>
				</div>
			</section>
		</div>
	);
}

function previewForMode(entry: UpdateMenuEntry, mode: UpdateMode) {
	if (
		entry.existing_preview.mode.target_type === mode.target_type &&
		entry.existing_preview.mode.mode === mode.mode
	) {
		return entry.existing_preview;
	}
	if (
		entry.add_new_preview?.mode.target_type === mode.target_type &&
		entry.add_new_preview.mode.mode === mode.mode
	) {
		return entry.add_new_preview;
	}
	return null;
}

interface UpdateTargetMenuProps<T extends UpdateMenuEntry> {
	entries: T[];
	filter: UpdateTargetFilter;
	modes: Record<string, UpdateMode>;
	busyKey: string | null;
	error: string | null;
	onFilter: (filter: UpdateTargetFilter) => void;
	onMode: (key: string, mode: UpdateMode) => void;
	onApply: (entry: T, mode: UpdateMode) => void;
	onCancel: () => void;
}

function UpdateTargetRow<T extends UpdateMenuEntry>({
	entry,
	filter,
	mode,
	busyKey,
	onMode,
	onApply,
}: {
	entry: T;
	filter: UpdateTargetFilter;
	mode: UpdateMode;
	busyKey: string | null;
	onMode: (key: string, mode: UpdateMode) => void;
	onApply: (entry: T, mode: UpdateMode) => void;
}) {
	const key = updateTargetKey(entry.target);
	const preview = previewForMode(entry, mode);
	const stats = preview
		? updatePreviewStats(preview)
		: { eligible: 0, changed: 0, ignored: 0 };
	const options = [entry.existing_preview, entry.add_new_preview]
		.filter((candidate): candidate is UpdatePreview => Boolean(candidate))
		.map((candidate) => ({
			value: JSON.stringify(candidate.mode),
			label: modeLabel(candidate.mode),
		}));
	return (
		<article
			className={`update-target-row ${stats.changed === 0 ? "no-op" : ""}`}
		>
			<div>
				<b>{entry.target.name}</b>
				<span>{updateTargetContext(entry.target)}</span>
				<small>
					{stats.eligible} eligible ·{" "}
					{stats.changed ? `${stats.changed} changes` : "No eligible change"}
					{stats.ignored ? ` · ${stats.ignored} ignored` : ""}
				</small>
			</div>
			{filter === "show_all_active" && (
				<SelectField
					label={`Mode for ${entry.target.name}`}
					value={JSON.stringify(mode)}
					onChange={(value) => onMode(key, JSON.parse(value) as UpdateMode)}
					options={options}
				/>
			)}
			<Button
				className="primary"
				disabled={busyKey != null || stats.changed === 0}
				onClick={() => onApply(entry, mode)}
			>
				{busyKey === key
					? "Updating…"
					: stats.changed === 0
						? "No changes"
						: "Update"}
			</Button>
		</article>
	);
}

export function UpdateTargetMenu<T extends UpdateMenuEntry>({
	entries,
	filter,
	modes,
	busyKey,
	error,
	onFilter,
	onMode,
	onApply,
	onCancel,
}: UpdateTargetMenuProps<T>) {
	return (
		<div
			className="modal-backdrop update-workflow-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onCancel()
			}
		>
			<section
				className="modal-card update-target-menu workflow-theme update-workflow"
				role="dialog"
				aria-modal="true"
				aria-label="Update Update"
			>
				<Button
					className="modal-close"
					aria-label="Close Update Update"
					onClick={onCancel}
				>
					×
				</Button>
				<h2>
					<span className="workflow-badge">UPDATE</span> Update Update
				</h2>
				<p>
					Choose an active or referenced target related to the current
					programmer changes.
				</p>
				<section
					className="segmented-control"
					aria-label="Eligible target filter"
				>
					<Button
						className={
							filter === "eligible_for_update_existing" ? "active" : ""
						}
						onClick={() => onFilter("eligible_for_update_existing")}
					>
						Eligible for Update Existing
					</Button>
					<Button
						className={filter === "show_all_active" ? "active" : ""}
						onClick={() => onFilter("show_all_active")}
					>
						Show All Active
					</Button>
				</section>
				<div className="update-target-list">
					{entries.length === 0 && (
						<p className="update-no-op">No targets match this filter.</p>
					)}
					{entries.map((entry) => {
						const key = updateTargetKey(entry.target);
						return (
							<UpdateTargetRow
								key={key}
								entry={entry}
								filter={filter}
								mode={modes[key] ?? entry.existing_preview.mode}
								busyKey={busyKey}
								onMode={onMode}
								onApply={onApply}
							/>
						);
					})}
				</div>
				{error && (
					<p className="modal-error" role="alert">
						{error}
					</p>
				)}
				<div className="modal-actions">
					<Button onClick={onCancel}>Cancel</Button>
				</div>
			</section>
		</div>
	);
}

export function UpdateResultDialog({
	result,
	onClose,
}: {
	result: UpdateResult;
	onClose: () => void;
}) {
	return (
		<div className="modal-backdrop update-workflow-layer">
			<section
				className="modal-card update-result-modal workflow-theme update-workflow"
				role="dialog"
				aria-modal="true"
				aria-label="Update complete"
			>
				<h2>
					<span className="workflow-badge">UPDATE</span> Update complete
				</h2>
				<p>
					<b>
						{targetFamilyLabel(result.target)} · {result.target.name}
					</b>
				</p>
				<p>{updateTargetContext(result.target)}</p>
				<div className="update-preview-summary">
					<span>Changed {result.changed_count}</span>
					<span>Added {result.added_count}</span>
					<span>Ineligible {result.ignored_count}</span>
					<span>
						Revision {result.revision_before} → {result.revision_after}
					</span>
				</div>
				{result.changed_cues.length > 0 && (
					<p>
						Changed Cue/source events:{" "}
						{result.changed_cues
							.map((cue) => `Cue ${cue.cue_number}`)
							.join(", ")}
						.
					</p>
				)}
				<p>
					{result.programmer_values_retained
						? "Programmer values were retained."
						: "Eligible programmer values were cleared."}
				</p>
				<div className="modal-actions">
					<Button className="primary" onClick={onClose}>
						Close
					</Button>
				</div>
			</section>
		</div>
	);
}
