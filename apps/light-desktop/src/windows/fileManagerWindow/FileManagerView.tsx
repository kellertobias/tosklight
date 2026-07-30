import { SelectiveShowImportModal } from "../../components/modals/SelectiveShowImportModal";
import { StackedModal } from "../../components/modals/StackedModal";
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
			{controller.partialImport.dialog &&
				controller.partialImport.activeShow &&
				controller.partialImport.capability && (
					<StackedModal onClose={controller.partialImport.close}>
						<SelectiveShowImportModal
							activeShow={controller.partialImport.activeShow}
							shows={[controller.partialImport.dialog.source]}
							initialSourceShowId={controller.partialImport.dialog.source.id}
							initialCatalog={controller.partialImport.dialog.catalog}
							onClose={controller.partialImport.close}
							loadCatalog={controller.partialImport.capability.catalog}
							previewImport={controller.partialImport.capability.preview}
							applyImport={controller.partialImport.capability.apply}
						/>
					</StackedModal>
				)}
		</section>
	);
}
