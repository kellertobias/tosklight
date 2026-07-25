import { createPortal } from "react-dom";
import { Button, Select } from "../../components/common";
import type { TextEditorController } from "./controller";

function saveDisabled(controller: TextEditorController) {
	return (
		!controller.document ||
		!controller.dirty ||
		controller.paneReadOnly ||
		controller.document.read_only ||
		controller.saving ||
		Boolean(controller.externalDocument) ||
		controller.availability === "missing"
	);
}

export function TextEditorPaneChrome({
	controller,
}: {
	controller: TextEditorController;
}) {
	const state = (
		<span
			className="text-editor-header-state"
			title={`${controller.status} · ${controller.label}`}
		>
			<strong
				className={`text-save-state ${controller.dirty || controller.externalDocument ? "dirty" : ""}`}
				role="status"
				aria-live="polite"
			>
				{controller.status}
			</strong>{" "}
			· {controller.label}
		</span>
	);
	const actions = (
		<div className="text-editor-header-actions">
			<Button onClick={() => void controller.openFile()}>Open File</Button>
			<Button
				disabled={!controller.selectedRoot || controller.filesLoading}
				onClick={() => void controller.reloadFiles(controller.selectedRoot)}
			>
				Refresh
			</Button>
			<Button disabled={saveDisabled(controller)} onClick={controller.save}>
				Save
			</Button>
			<Button
				aria-label="Save As"
				disabled={
					!controller.selectedRoot ||
					controller.paneReadOnly ||
					controller.saving
				}
				onClick={controller.saveAs}
			>
				Save As…
			</Button>
		</div>
	);
	return (
		<>
			{controller.paneChrome?.info &&
				createPortal(state, controller.paneChrome.info)}
			{controller.paneChrome?.toolbar &&
				createPortal(actions, controller.paneChrome.toolbar)}
		</>
	);
}

export function TextEditorToolbar({
	controller,
}: {
	controller: TextEditorController;
}) {
	return (
		<div className="text-editor-toolbar">
			{!controller.paneChrome && (
				<Button onClick={() => void controller.openFile()}>Open File</Button>
			)}
			<Select
				aria-label="File root"
				value={controller.selectedRoot}
				onChange={(event) => controller.associateFile(event.target.value, "")}
			>
				{controller.roots.map((root) => (
					<option key={root.id} value={root.id}>
						{root.label}
						{root.writable ? "" : " (read-only)"}
					</option>
				))}
			</Select>
			<Select
				aria-label="Choose File"
				value={controller.selectedPath}
				disabled={!controller.selectedRoot || controller.filesLoading}
				onChange={(event) =>
					controller.associateFile(controller.selectedRoot, event.target.value)
				}
			>
				<option value="">
					{controller.filesLoading ? "Loading text files…" : "Choose File…"}
				</option>
				{controller.chooserFiles.map((file) => (
					<option key={file.path} value={file.path}>
						{file.path}
						{file.writable ? "" : " (read-only or missing)"}
					</option>
				))}
			</Select>
			{!controller.paneChrome && <StandaloneActions controller={controller} />}
			<Button
				disabled={!controller.selectedPath}
				onClick={() => controller.associateFile(controller.selectedRoot, "")}
			>
				Close File
			</Button>
		</div>
	);
}

function StandaloneActions({
	controller,
}: {
	controller: TextEditorController;
}) {
	return (
		<>
			<strong
				className={`text-save-state ${controller.dirty || controller.externalDocument ? "dirty" : ""}`}
				role="status"
				aria-live="polite"
			>
				{controller.status}
			</strong>
			<span title={controller.label}>{controller.label}</span>
			<Button disabled={saveDisabled(controller)} onClick={controller.save}>
				Save
			</Button>
			<Button
				aria-label="Save As"
				disabled={
					!controller.selectedRoot ||
					controller.paneReadOnly ||
					controller.saving
				}
				onClick={controller.saveAs}
			>
				Save As…
			</Button>
			<Button
				disabled={!controller.selectedRoot || controller.filesLoading}
				onClick={() => void controller.reloadFiles(controller.selectedRoot)}
			>
				Refresh
			</Button>
		</>
	);
}
