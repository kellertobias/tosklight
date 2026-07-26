import type { ReactNode } from "react";
import { ControlSection } from "../control/ControlSection";
import { CommandChoiceModal } from "../modals/CommandChoiceModal";
import { DebugModal } from "../modals/DebugModal";
import { PreloadStoreModal } from "../modals/PreloadStoreModal";
import { QuickSetupModal } from "../modals/QuickSetupModal";
import { ShowRecoveryModal } from "../modals/ShowRecoveryModal";
import { SpecialDialogsModal } from "../modals/SpecialDialogsModal";
import { StoreSettingsModal } from "../modals/StoreSettingsModal";
import { SystemControlsModal } from "../modals/SystemControlsModal";
import { UpdateWorkflow } from "../modals/UpdateWorkflow";
import { ConnectionState } from "./ConnectionState";
import { DeskLoadingOverlay } from "./DeskLoadingOverlay";
import { LayoutPersistence } from "./LayoutPersistence";
import { LeftDock } from "./LeftDock";
import { NativeDragStrip } from "./NativeDragStrip";
import { ScreenWindowManager } from "./ScreenWindowManager";
import { SectionNameMap } from "./SectionNameMap";
import { WorkspaceView } from "./WorkspaceView";

export function AppShell() {
	return (
		<AppShellView
			nativeDragStrip={<NativeDragStrip />}
			sectionNameMap={<SectionNameMap />}
			layoutPersistence={<LayoutPersistence />}
			screenWindowManager={<ScreenWindowManager />}
			dock={<LeftDock />}
			workspace={<WorkspaceView />}
			control={<ControlSection />}
			modals={
				<>
					<QuickSetupModal />
					<DebugModal />
					<SpecialDialogsModal />
					<SystemControlsModal />
					<PreloadStoreModal />
					<StoreSettingsModal />
					<UpdateWorkflow />
					<CommandChoiceModal />
					<ShowRecoveryModal />
				</>
			}
			connectionState={<ConnectionState />}
			loadingOverlay={<DeskLoadingOverlay />}
		/>
	);
}

export interface AppShellViewProps {
	nativeDragStrip?: ReactNode;
	sectionNameMap?: ReactNode;
	layoutPersistence?: ReactNode;
	screenWindowManager?: ReactNode;
	dock: ReactNode;
	workspace: ReactNode;
	control: ReactNode;
	modals?: ReactNode;
	connectionState?: ReactNode;
	loadingOverlay?: ReactNode;
}

/**
 * The production shell composition boundary. Runtime ownership stays in AppShell while
 * deterministic renderers can supply the same Dock, workspace, control, and overlay slots.
 */
export function AppShellView({
	nativeDragStrip,
	sectionNameMap,
	layoutPersistence,
	screenWindowManager,
	dock,
	workspace,
	control,
	modals,
	connectionState,
	loadingOverlay,
}: AppShellViewProps) {
	return (
		<div
			className="app-shell"
			data-light-surface="application"
			aria-label="ToskLight application"
			role="application"
		>
			{nativeDragStrip}
			{sectionNameMap}
			{layoutPersistence}
			{screenWindowManager}
			{dock}
			{workspace}
			{control}
			{modals}
			{connectionState}
			{loadingOverlay}
		</div>
	);
}
