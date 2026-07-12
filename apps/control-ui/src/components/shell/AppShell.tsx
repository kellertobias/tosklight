import { LeftDock } from "./LeftDock";
import { WorkspaceView } from "./WorkspaceView";
import { ControlSection } from "../control/ControlSection";
import { QuickSetupModal } from "../modals/QuickSetupModal";
import { SpecialDialogsModal } from "../modals/SpecialDialogsModal";
import { LayoutPersistence } from "./LayoutPersistence";
import { ConnectionState } from "./ConnectionState";
import { SystemControlsModal } from "../modals/SystemControlsModal";
import { PreloadStoreModal } from "../modals/PreloadStoreModal";

export function AppShell() {
  return <div className="app-shell">
    <LayoutPersistence />
    <LeftDock />
    <WorkspaceView />
    <ControlSection />
    <QuickSetupModal />
    <SpecialDialogsModal />
    <SystemControlsModal />
    <PreloadStoreModal />
    <ConnectionState />
  </div>;
}
