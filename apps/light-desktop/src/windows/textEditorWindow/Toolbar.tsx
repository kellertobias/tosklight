import { createPortal } from "react-dom";
import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
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
			title={controller.label}
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
	if (!controller.paneChrome) {
		return (
			<WindowHeader
				title="Text Editor"
				info={{ primary: state }}
				toolbar={actions}
			/>
		);
	}
	return (
		<>
			{controller.paneChrome?.info &&
				createPortal(state, controller.paneChrome.info)}
			{controller.paneChrome?.toolbar &&
				createPortal(actions, controller.paneChrome.toolbar)}
		</>
	);
}
