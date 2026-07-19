import {
	Button,
	CheckboxField,
	TextArea,
	TextInput,
} from "../../components/common";
import { validItemName } from "./fileUtilities";
import type { FileManagerController } from "./useFileManagerController";

function RenameEditor({ controller }: { controller: FileManagerController }) {
	const { state, operations } = controller;
	const operation = state.operation;
	if (operation?.kind !== "rename" || operation.sources.length !== 1)
		return null;
	return (
		<section
			className="file-operation-panel file-rename-editor"
			aria-label="Rename item"
		>
			<strong>Rename {operation.sources[0].entry.name}</strong>
			<TextInput
				aria-label="New name"
				autoFocus
				value={operation.renameDraft}
				onChange={(event) =>
					operations.setOperation({
						...operation,
						renameDraft: event.target.value,
					})
				}
				onKeyDown={(event) => {
					if (event.key === "Enter") {
						event.preventDefault();
						void operations.completeOperation();
					}
					if (event.key === "Escape") {
						event.preventDefault();
						operations.cancelOperation();
					}
				}}
			/>
			{!validItemName(operation.renameDraft) && (
				<span role="alert">Enter a name without path separators.</span>
			)}
		</section>
	);
}

function OperationConfirmations({
	controller,
}: {
	controller: FileManagerController;
}) {
	const { state, operations, conflicts } = controller;
	const operation = state.operation;
	const conflict = state.conflict;
	return (
		<>
			{operation?.kind === "delete" && operation.confirming && (
				<section
					className="file-confirmation"
					role="dialog"
					aria-modal="true"
					aria-label={
						controller.trashForOperation
							? "Confirm move to trash"
							: "Confirm permanent deletion"
					}
				>
					<h3>
						Delete {operation.sources.length} item
						{operation.sources.length === 1 ? "" : "s"}?
					</h3>
					<p>
						{controller.trashForOperation
							? "The selected item(s) will be moved to the platform Trash."
							: "Trash is unavailable on this filesystem. This deletion is permanent."}
					</p>
					<div>
						<Button
							variant="primary"
							disabled={state.busy}
							onClick={() => void operations.completeOperation()}
						>
							{controller.trashForOperation
								? "Move to Trash"
								: "Delete Permanently"}
						</Button>
						<Button
							disabled={state.busy}
							onClick={() => operations.cancelOperation()}
						>
							Cancel
						</Button>
					</div>
				</section>
			)}
			{conflict && (
				<section
					className="file-confirmation"
					role="dialog"
					aria-modal="true"
					aria-label="Resolve name conflict"
				>
					<h3>An item with that name already exists</h3>
					<p>Choose how this conflict should be handled.</p>
					{conflict.operation.sources.length > 1 && (
						<CheckboxField
							label="Apply to All"
							checked={conflict.applyToAll}
							onChange={(event) =>
								state.setConflict({
									...conflict,
									applyToAll: event.target.checked,
								})
							}
						/>
					)}
					<div>
						<Button
							variant="primary"
							disabled={state.busy}
							onClick={() => void conflicts.resolveReplace()}
						>
							Replace
						</Button>
						<Button
							disabled={state.busy}
							onClick={() => void conflicts.resolveKeepBoth()}
						>
							Keep Both
						</Button>
						<Button
							disabled={state.busy}
							onClick={() => void conflicts.resolveSkip()}
						>
							Skip
						</Button>
						<Button
							disabled={state.busy}
							onClick={() => operations.cancelOperation()}
						>
							Cancel
						</Button>
					</div>
				</section>
			)}
		</>
	);
}

function TextEditor({ controller }: { controller: FileManagerController }) {
	const { state, editor } = controller;
	if (!state.editor) return null;
	const status = state.editorMissing
		? "Missing"
		: state.editorConflict
			? "Conflict"
			: state.editorText !== state.editor.text
				? "Unsaved"
				: "Saved";
	return (
		<div className="file-editor">
			<header>
				<b>{state.editor.path}</b>
				<span role="status">{status}</span>
				<Button
					disabled={
						state.editorText === state.editor.text ||
						state.editor.read_only ||
						state.busy ||
						Boolean(state.editorConflict) ||
						state.editorMissing
					}
					onClick={() => void editor.saveText()}
				>
					Save
				</Button>
				{state.editorMissing && (
					<Button
						disabled={state.busy}
						onClick={() => void editor.recreateText()}
					>
						Recreate File
					</Button>
				)}
				<Button onClick={editor.closeText}>Close</Button>
			</header>
			{state.editorConflict && (
				<div className="file-message" role="alert">
					A newer file revision is available. Your unsaved text has not been
					overwritten.{" "}
					<Button onClick={editor.reloadConflict}>Reload Newer Version</Button>
				</div>
			)}
			{state.editorMissing && (
				<div className="file-message" role="alert">
					The associated file is missing. The last loaded text is retained until
					you recreate or close it.
				</div>
			)}
			<TextArea
				aria-label="File text"
				value={state.editorText}
				readOnly={state.editor.read_only || state.editorMissing}
				onChange={(event) => state.setEditorText(event.target.value)}
			/>
		</div>
	);
}

export function FileManagerDialogs({
	controller,
}: {
	controller: FileManagerController;
}) {
	return (
		<>
			<RenameEditor controller={controller} />
			<OperationConfirmations controller={controller} />
			<TextEditor controller={controller} />
		</>
	);
}
