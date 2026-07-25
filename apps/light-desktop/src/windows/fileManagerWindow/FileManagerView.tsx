import { FileManagerBrowser } from "./FileManagerBrowser";
import { FileManagerDialogs } from "./FileManagerDialogs";
import { FileManagerHeader } from "./FileManagerHeader";
import type { FileManagerController } from "./useFileManagerController";

export function FileManagerView({
	controller,
}: {
	controller: FileManagerController;
}) {
	const { state } = controller;
	return (
		<section
			className={`file-manager fm-${state.view} fm-${state.sidePanel}-open ${state.propertiesVisible ? "fm-properties-visible" : "fm-properties-hidden"}`}
			aria-label={controller.picker ? "File picker" : "File Manager"}
			data-file-manager-instance={state.instanceId}
			onPointerDownCapture={controller.operations.claimPendingAction}
		>
			<FileManagerHeader controller={controller} />
			<FileManagerBrowser controller={controller} />
			<FileManagerDialogs controller={controller} />
		</section>
	);
}
