import { PlaybackConfigurationRequestHost } from "../control/PlaybackConfigurationRequestHost";
import { CommandChoiceModal } from "../modals/CommandChoiceModal";
import { DebugModal } from "../modals/DebugModal";
import { PreloadStoreModal } from "../modals/PreloadStoreModal";
import { QuickSetupModal } from "../modals/QuickSetupModal";
import { ShowRecoveryModal } from "../modals/ShowRecoveryModal";
import { SpecialDialogsModal } from "../modals/SpecialDialogsModal";
import { StoreSettingsModal } from "../modals/StoreSettingsModal";
import { RecordChoiceHost } from "../modals/RecordChoiceHost";
import { SystemControlsModal } from "../modals/SystemControlsModal";
import { UpdateWorkflow } from "../modals/UpdateWorkflow";

/** Shared desk workflows for Main and attached Screen surfaces. */
export function DeskModalHost() {
	return (
		<>
			<PlaybackConfigurationRequestHost />
			<QuickSetupModal />
			<DebugModal />
			<SpecialDialogsModal />
			<SystemControlsModal />
			<PreloadStoreModal />
			<StoreSettingsModal />
			<RecordChoiceHost />
			<UpdateWorkflow />
			<CommandChoiceModal />
			<ShowRecoveryModal />
		</>
	);
}
