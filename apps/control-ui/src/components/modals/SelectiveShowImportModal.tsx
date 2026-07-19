import { type MutableRefObject, useLayoutEffect } from "react";
import type { SelectiveImportOutcome } from "../../api/selectiveImportModels";
import { Button, ModalTitleBar } from "../common";
import {
	CatalogSelection,
	PreviewDetails,
} from "./SelectiveImportPreviewDetails";
import {
	type SelectiveImportWorkflow,
	type SelectiveImportWorkflowOptions,
	useSelectiveImportWorkflow,
} from "./selectiveImportWorkflow";

export interface SelectiveShowImportModalProps extends SelectiveImportWorkflowOptions {
	closeTriggerRef?: MutableRefObject<(() => void) | null>;
}

export function SelectiveShowImportModal(props: SelectiveShowImportModalProps) {
	const workflow = useSelectiveImportWorkflow(props);
	useCloseTrigger(props.closeTriggerRef, workflow.close);
	if (workflow.outcome) {
		return <ImportComplete
			activeShowName={props.activeShow.name}
			outcome={workflow.outcome}
			onClose={workflow.close}
		/>;
	}
	return (
		<section className="nested-modal" role="dialog" aria-modal="true" aria-label="Partial Show Load">
			<ModalTitleBar
				title="Partial Show Load"
				closeLabel="Close Partial Show Load"
				onClose={workflow.close}
			/>
			<p>
				Choose content from another show. The Selective Show Import preview lists every
				dependency, conflict, fixture profile, and managed asset before changing the active show.
			</p>
			<SourceShowSelector workflow={workflow} />
			{workflow.catalog && <CatalogSelection
				catalog={workflow.catalog}
				selected={workflow.selected}
				disabled={workflow.phase !== "idle"}
				onChange={workflow.toggleObject}
			/>}
			{workflow.preview && <PreviewDetails
				preview={workflow.preview}
				disabled={workflow.phase !== "idle"}
				objectChoices={workflow.objectChoices}
				profileChoices={workflow.profileChoices}
				onObjectChoice={workflow.setObjectChoice}
				onProfileChoice={workflow.setProfileChoice}
			/>}
			<WorkflowStatus workflow={workflow} />
			<WorkflowActions workflow={workflow} />
		</section>
	);
}

function useCloseTrigger(
	trigger: MutableRefObject<(() => void) | null> | undefined,
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

function ImportComplete({ activeShowName, outcome, onClose }: {
	activeShowName: string;
	outcome: SelectiveImportOutcome;
	onClose: () => void;
}) {
	const message = outcome.changed
		? `Imported ${outcome.objectChanges.length} object changes into ${activeShowName} as one show revision.`
		: "The selected content was already identical. The show was not changed.";
	return <section
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
		<Button variant="primary" onClick={onClose}>Done</Button>
	</section>;
}

function SourceShowSelector({ workflow }: { workflow: SelectiveImportWorkflow }) {
	return <label>
		Source show
		<select
			aria-label="Source show"
			value={workflow.sourceId}
			disabled={workflow.phase !== "idle"}
			onChange={(event) => void workflow.chooseSource(event.target.value)}
		>
			<option value="">Choose a show…</option>
			{workflow.sources.map((show) => (
				<option key={show.id} value={show.id}>{show.name}</option>
			))}
		</select>
	</label>;
}

function WorkflowStatus({ workflow }: { workflow: SelectiveImportWorkflow }) {
	return <>
		{workflow.phase === "catalog" && <p role="status">Reading the source show…</p>}
		{workflow.phase === "preview" && (
			<p role="status">Building the dependency and conflict preview…</p>
		)}
		{workflow.phase === "apply" && (
			<p role="status">Importing atomically… The write cannot be cancelled after it starts.</p>
		)}
		{workflow.error && <p className="modal-error" role="alert">{workflow.error}</p>}
	</>;
}

function WorkflowActions({ workflow }: { workflow: SelectiveImportWorkflow }) {
	const retrySource = workflow.error && workflow.sourceId && !workflow.catalog;
	const noSelection = workflow.selection.selectedObjects.length === 0;
	return <footer className="modal-actions">
		<Button disabled={workflow.phase === "apply"} onClick={workflow.close}>Cancel</Button>
		{retrySource && (
			<Button onClick={() => void workflow.chooseSource(workflow.sourceId)}>Retry Source</Button>
		)}
		<Button
			disabled={workflow.phase !== "idle" || noSelection}
			onClick={() => void workflow.inspectSelection()}
		>
			{workflow.preview ? "Update Preview" : workflow.error ? "Retry Preview" : "Preview Import"}
		</Button>
		<Button
			variant="primary"
			disabled={workflow.phase !== "idle" || !workflow.previewCurrent || !workflow.preview?.canApply}
			onClick={() => void workflow.apply()}
		>
			Apply as One Show Revision
		</Button>
	</footer>;
}
