import { Button, TextArea } from "../../components/common";
import type { TextEditorController } from "./controller";

export function TextEditorMessages({
	controller,
}: {
	controller: TextEditorController;
}) {
	return (
		<>
			{controller.paneReadOnly && (
				<div className="file-message text-editor-read-only" role="status">
					This pane is configured read-only. Editing, Save, Save As, import, and
					recreate actions are disabled.
				</div>
			)}
			{controller.notice && (
				<div
					id={controller.messageId}
					className={`file-message text-editor-${controller.notice.kind}`}
					role={controller.notice.kind === "info" ? "status" : "alert"}
				>
					{controller.notice.text}
				</div>
			)}
			{controller.availability === "missing" && controller.selectedPath && (
				<MissingFileActions controller={controller} />
			)}
			{controller.externalDocument && (
				<RevisionConflict controller={controller} />
			)}
		</>
	);
}

function MissingFileActions({
	controller,
}: {
	controller: TextEditorController;
}) {
	return (
		<section
			className="file-message text-editor-missing-actions"
			aria-label="Missing file actions"
		>
			<Button
				disabled={controller.saving || controller.paneReadOnly}
				onClick={controller.recreate}
			>
				Recreate File
			</Button>
			<Button
				disabled={controller.saving || controller.paneReadOnly}
				onClick={controller.saveAs}
			>
				Save Retained Text As…
			</Button>
			<Button
				disabled={controller.filesLoading}
				onClick={() => void controller.reloadFiles(controller.selectedRoot)}
			>
				Look for Moved File
			</Button>
		</section>
	);
}

function RevisionConflict({
	controller,
}: {
	controller: TextEditorController;
}) {
	const external = controller.externalDocument;
	if (!external) return null;
	return (
		<section
			className="file-message text-editor-conflict"
			aria-label="File revision conflict"
		>
			<b>A newer file revision is available.</b>
			<Button onClick={controller.reloadExternal}>Reload Newer Version</Button>
			<Button
				disabled={controller.saving || controller.paneReadOnly}
				onClick={controller.saveAs}
			>
				Save My Version As…
			</Button>
			<details>
				<summary>Compare versions</summary>
				{/* biome-ignore lint/a11y/noLabelWithoutControl: TextArea renders the native control within this comparison label. */}
				<label>
					Your unsaved version
					<TextArea
						aria-label="Your unsaved version"
						value={controller.text}
						readOnly
					/>
				</label>
				{/* biome-ignore lint/a11y/noLabelWithoutControl: TextArea renders the native control within this comparison label. */}
				<label>
					Newer file version
					<TextArea
						aria-label="Newer file version"
						value={external.text}
						readOnly
					/>
				</label>
			</details>
		</section>
	);
}
