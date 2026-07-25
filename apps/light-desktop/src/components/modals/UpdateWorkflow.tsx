import {
	UpdateOperationDialog,
	UpdateResultDialog,
	UpdateSettingsDialog,
	UpdateTargetMenu,
} from "./UpdateWorkflowDialogs";
import { useUpdateWorkflowController } from "./useUpdateWorkflowController";

export {
	UpdateOperationDialog,
	UpdateSettingsDialog,
	UpdateTargetMenu,
	updatePreviewStats,
} from "./UpdateWorkflowDialogs";

export function UpdateWorkflow() {
	const workflow = useUpdateWorkflowController();
	return (
		<>
			{workflow.armed && !workflow.operation && !workflow.busy && (
				<div className="update-armed-banner" role="status">
					UPDATE armed · touch a recordable target or enter its address
				</div>
			)}
			{workflow.busy &&
				!workflow.operation &&
				!workflow.settingsOpen &&
				!workflow.menu.open && (
					<div className="update-armed-banner busy" role="status">
						Resolving authoritative Update target…
					</div>
				)}
			{workflow.operation && (
				<UpdateOperationDialog
					operation={workflow.operation}
					busy={workflow.busy}
					error={workflow.localError}
					onMode={(mode) => void workflow.changeOperationMode(mode)}
					onApply={() => void workflow.applyOperation()}
					onCancel={workflow.cancelOperation}
				/>
			)}
			{workflow.settingsOpen && (
				<UpdateSettingsDialog
					settings={workflow.settings}
					busy={workflow.busy}
					error={workflow.localError}
					onChange={workflow.setSettings}
					onSave={() => void workflow.saveSettings()}
					onCancel={workflow.cancelSettings}
				/>
			)}
			{workflow.menu.open && (
				<UpdateTargetMenu
					entries={workflow.menu.entries}
					filter={workflow.menu.filter}
					modes={workflow.menu.modes}
					busyKey={workflow.menu.busyKey}
					error={workflow.localError}
					onFilter={(filter) => void workflow.menu.load(filter)}
					onMode={workflow.menu.setMode}
					onApply={(entry, mode) => void workflow.applyMenuTarget(entry, mode)}
					onCancel={workflow.menu.close}
				/>
			)}
			{workflow.result && (
				<UpdateResultDialog
					result={workflow.result}
					onClose={workflow.closeResult}
				/>
			)}
		</>
	);
}
