import {
	Button,
	ErrorAlert,
	ModalTitleBar,
	OperationBusyOverlay,
	RadioField,
	SelectField,
} from "@tosklight/ui";
import { type RefObject, useLayoutEffect } from "react";
import type { SelectiveImportOutcome } from "../../api/selectiveImportModels";
import {
	CatalogSelection,
	PreviewDetails,
} from "./SelectiveImportPreviewDetails";
import {
	type SelectiveImportWorkflow,
	type SelectiveImportWorkflowOptions,
	useSelectiveImportWorkflow,
} from "./selectiveImportWorkflow";

export interface SelectiveShowImportModalProps
	extends SelectiveImportWorkflowOptions {
	closeTriggerRef?: RefObject<(() => void) | null>;
}

export function SelectiveShowImportModal(props: SelectiveShowImportModalProps) {
	const workflow = useSelectiveImportWorkflow(props);
	useCloseTrigger(props.closeTriggerRef, workflow.close);
	if (workflow.outcome) {
		return (
			<ImportComplete
				activeShowName={props.activeShow.name}
				outcome={workflow.outcome}
				onClose={workflow.close}
			/>
		);
	}
	return (
		<section
			className="nested-modal"
			role="dialog"
			aria-modal="true"
			aria-label="Partial Show Load"
		>
			<ModalTitleBar
				title="Partial Show Load"
				closeLabel="Close Partial Show Load"
				closeDisabled={workflow.phase === "apply"}
				onClose={workflow.close}
			/>
			<p>
				Choose content from another show. The Selective Show Import preview
				lists every dependency, conflict, fixture profile, and managed asset
				before changing the active show.
			</p>
			<SourceShowSelector workflow={workflow} />
			<LoadModeSelector workflow={workflow} />
			{workflow.catalog && (
				<CatalogSelection
					catalog={workflow.catalog}
					selected={workflow.selected}
					disabled={workflow.phase !== "idle"}
					onChange={workflow.toggleObject}
				/>
			)}
			{workflow.preview && (
				<PreviewDetails
					preview={workflow.preview}
					disabled={workflow.phase !== "idle"}
					objectChoices={workflow.objectChoices}
					profileChoices={workflow.profileChoices}
					onObjectChoice={workflow.setObjectChoice}
					onProfileChoice={workflow.setProfileChoice}
				/>
			)}
			<WorkflowStatus workflow={workflow} />
			<WorkflowActions workflow={workflow} />
		</section>
	);
}

function LoadModeSelector({ workflow }: { workflow: SelectiveImportWorkflow }) {
	return (
		<fieldset>
			<legend>Load mode</legend>
			<RadioField
				label="Replace by position"
				name="selective-import-mode"
				value="replace_by_position"
				checked={workflow.mode === "replace_by_position"}
				disabled={workflow.phase !== "idle"}
				onChange={() => workflow.setMode("replace_by_position")}
			/>
			<RadioField
				label="Add to end"
				name="selective-import-mode"
				value="add_to_end"
				checked={workflow.mode === "add_to_end"}
				disabled={workflow.phase !== "idle"}
				onChange={() => workflow.setMode("add_to_end")}
			/>
		</fieldset>
	);
}

function useCloseTrigger(
	trigger: RefObject<(() => void) | null> | undefined,
	close: () => void,
) {
	useLayoutEffect(() => {
		if (!trigger) return;
		trigger.current = close;
		return () => {
			if (trigger.current === close) trigger.current = null;
		};
	}, [trigger, close]);
}

function ImportComplete({
	activeShowName,
	outcome,
	onClose,
}: {
	activeShowName: string;
	outcome: SelectiveImportOutcome;
	onClose: () => void;
}) {
	const message = outcome.changed
		? `Imported ${outcome.objectChanges.length} object changes into ${activeShowName} as one show revision and one operator Undo step.`
		: "The selected content was already identical. The show was not changed.";
	return (
		<section
			className="nested-modal"
			role="dialog"
			aria-modal="true"
			aria-label="Partial Show Load complete"
		>
			<ModalTitleBar
				title="Partial Show Load Complete"
				closeLabel="Close Partial Show Load"
				onClose={onClose}
			/>
			<p role="status">{message}</p>
			<Button variant="primary" onClick={onClose}>
				Done
			</Button>
		</section>
	);
}

function SourceShowSelector({
	workflow,
}: {
	workflow: SelectiveImportWorkflow;
}) {
	return (
		<SelectField
			label="Source show"
			ariaLabel="Source show"
			value={workflow.sourceId}
			disabled={workflow.phase !== "idle"}
			onChange={(value) => void workflow.chooseSource(value)}
			options={[
				{ value: "", label: "Choose a show…" },
				...workflow.sources.map((show) => ({
					value: show.id,
					label: show.name,
				})),
			]}
		/>
	);
}

function WorkflowStatus({ workflow }: { workflow: SelectiveImportWorkflow }) {
	return (
		<>
			{workflow.phase !== "idle" && (
				<OperationBusyOverlay
					title={
						workflow.phase === "catalog"
							? "Reading the source show…"
							: workflow.phase === "preview"
								? "Checking selected show items…"
								: "Importing selected show items…"
					}
					message={
						workflow.phase === "apply"
							? "Saving the selected items to your show…"
							: "Preparing the import preview. The active show is unchanged."
					}
					onCancel={workflow.phase === "apply" ? undefined : workflow.close}
				/>
			)}
			{workflow.error && (
				<ErrorAlert as="p" className="modal-error" role="alert">
					{workflow.error}
				</ErrorAlert>
			)}
		</>
	);
}

function WorkflowActions({ workflow }: { workflow: SelectiveImportWorkflow }) {
	const retrySource = workflow.error && workflow.sourceId && !workflow.catalog;
	const noSelection = workflow.selection.selectedObjects.length === 0;
	return (
		<footer className="modal-actions">
			<Button disabled={workflow.phase === "apply"} onClick={workflow.close}>
				Cancel
			</Button>
			{retrySource && (
				<Button onClick={() => void workflow.chooseSource(workflow.sourceId)}>
					Retry Source
				</Button>
			)}
			<Button
				disabled={workflow.phase !== "idle" || noSelection}
				onClick={() => void workflow.inspectSelection()}
			>
				{workflow.preview
					? "Update Preview"
					: workflow.error
						? "Retry Preview"
						: "Preview Import"}
			</Button>
			<Button
				variant="primary"
				disabled={
					workflow.phase !== "idle" ||
					!workflow.previewCurrent ||
					!workflow.preview?.canApply
				}
				onClick={() => void workflow.apply()}
			>
				Apply{" "}
				{workflow.mode === "add_to_end" ? "Add to end" : "Replace by position"}{" "}
				as One Undo Step
			</Button>
		</footer>
	);
}
