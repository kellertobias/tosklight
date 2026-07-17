import { LeftDock } from "./LeftDock";
import { WorkspaceView } from "./WorkspaceView";
import { ControlSection } from "../control/ControlSection";
import { QuickSetupModal } from "../modals/QuickSetupModal";
import { SpecialDialogsModal } from "../modals/SpecialDialogsModal";
import { LayoutPersistence } from "./LayoutPersistence";
import { ConnectionState } from "./ConnectionState";
import { SystemControlsModal } from "../modals/SystemControlsModal";
import { PreloadStoreModal } from "../modals/PreloadStoreModal";
import { ModalEscapeManager } from "../input/ModalEscapeManager";
import { StoreSettingsModal } from "../modals/StoreSettingsModal";
import { ShowRecoveryModal } from "../modals/ShowRecoveryModal";
import { ScreenWindowManager } from "./ScreenWindowManager";
import { NativeDragStrip } from "./NativeDragStrip";
import { SectionNameMap } from "./SectionNameMap";
import { DebugModal } from "../modals/DebugModal";
import { CommandChoiceModal } from "../modals/CommandChoiceModal";

export function AppShell() {
  return <div className="app-shell">
    <NativeDragStrip />
    <SectionNameMap />
    <LayoutPersistence />
    <ScreenWindowManager />
    <ModalEscapeManager />
    <LeftDock />
    <WorkspaceView />
    <ControlSection />
    <QuickSetupModal />
    <DebugModal />
    <SpecialDialogsModal />
    <SystemControlsModal />
    <PreloadStoreModal />
    <StoreSettingsModal />
    <CommandChoiceModal />
    <ConnectionState />
    <ShowRecoveryModal />
  </div>;
}
